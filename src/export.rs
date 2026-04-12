use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parser::parse_frontmatter;
use crate::registry::Registry;
use crate::security;
use crate::types::{KnowerageError, RegistryRecord};

/// Maximum number of analysis paths in one `generate_bundle` call (accidental huge lists).
pub const MAX_ANALYSIS_PATHS: usize = 10_000;

/// Maximum UTF-8 bytes per output `combined*.md` part (before starting `combined_2.md`, etc.).
pub const MAX_COMBINED_PART_BYTES: usize = 50 * 1024 * 1024;

/// Maximum UTF-8 bytes read from one analysis file (must be ≥ [`MAX_COMBINED_PART_BYTES`] so a
/// single analysis file does not exceed the combined file limit; larger files are rejected with `ExportError`).
pub const MAX_BYTES_PER_ANALYSIS_FILE: usize = MAX_COMBINED_PART_BYTES;

const COMBINED_SEPARATOR: &str = "---\n\n";

pub fn generate_report(
    records: &HashMap<String, RegistryRecord>,
    format: &str,
    output_path: &Path,
) -> Result<(), KnowerageError> {
    let content = match format {
        "json" => serde_json::to_string_pretty(records)
            .map_err(|e| KnowerageError::RegistryIo(format!("JSON serialization failed: {e}")))?,
        "yaml" => serde_yaml::to_string(records)
            .map_err(|e| KnowerageError::RegistryIo(format!("YAML serialization failed: {e}")))?,
        "txt" => generate_txt(records),
        "html" => generate_html(records),
        other => {
            return Err(KnowerageError::DocParse(format!(
                "Unsupported export format: '{other}'. Supported formats: json, yaml, txt, html"
            )))
        }
    };

    security::atomic_write(output_path, content.as_bytes())
}

fn generate_txt(records: &HashMap<String, RegistryRecord>) -> String {
    let mut lines = vec![
        "Knowerage Registry Report".to_string(),
        format!("Total records: {}", records.len()),
        String::new(),
    ];

    let mut sorted: Vec<_> = records.iter().collect();
    sorted.sort_by_key(|(k, _)| k.as_str());

    for (key, record) in sorted {
        let ranges = record
            .covered_ranges
            .iter()
            .map(|r| format!("{}-{}", r[0], r[1]))
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!(
            "{}|{}|{}",
            record.source_path.display(),
            key,
            ranges
        ));
        lines.push(format!("  Status: {}", record.status));
    }

    lines.join("\n")
}

fn generate_html(records: &HashMap<String, RegistryRecord>) -> String {
    let mut html = String::from(concat!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\n",
        "<title>Knowerage Report</title></head><body>\n",
        "<h1>Knowerage Registry Report</h1>\n"
    ));
    html.push_str(&format!("<p>Total records: {}</p>\n", records.len()));
    html.push_str(concat!(
        "<table border=\"1\"><tr>",
        "<th>Source</th><th>Analysis</th><th>Status</th><th>Ranges</th>",
        "</tr>\n"
    ));

    let mut sorted: Vec<_> = records.iter().collect();
    sorted.sort_by_key(|(k, _)| k.as_str());

    for (key, record) in sorted {
        let ranges = record
            .covered_ranges
            .iter()
            .map(|r| format!("{}-{}", r[0], r[1]))
            .collect::<Vec<_>>()
            .join(", ");
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            html_escape(&record.source_path.to_string_lossy()),
            html_escape(key),
            record.status,
            ranges
        ));
    }

    html.push_str("</table>\n</body></html>");
    html
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSelection {
    pub paths: Vec<PathBuf>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPartInfo {
    pub part_index: u32,
    pub toc_file: String,
    pub combined_file: String,
    pub combined_byte_length: usize,
    pub analysis_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub created_at: DateTime<Utc>,
    pub files: Vec<ExportFileEntry>,
    pub errors: Vec<ExportError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ExportPartInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFileEntry {
    pub analysis_path: PathBuf,
    pub source_path: PathBuf,
    pub content_hash: String,
    #[serde(default = "default_part_index")]
    pub part_index: u32,
}

fn default_part_index() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportError {
    pub path: PathBuf,
    pub error: String,
}

/// One chunk of TOC + combined markdown.
#[derive(Debug, Clone)]
pub struct ExportBundlePart {
    pub toc: String,
    pub combined: String,
}

#[derive(Debug)]
pub struct ExportBundle {
    pub parts: Vec<ExportBundlePart>,
    pub manifest: ExportManifest,
}

impl ExportBundle {
    /// First part’s combined body (empty string if no parts).
    pub fn primary_combined(&self) -> &str {
        self.parts
            .first()
            .map(|p| p.combined.as_str())
            .unwrap_or("")
    }

    /// First part’s TOC markdown.
    pub fn primary_toc(&self) -> &str {
        self.parts
            .first()
            .map(|p| p.toc.as_str())
            .unwrap_or("")
    }
}

pub fn select_files(
    selection: &ExportSelection,
    registry: &Registry,
    _workspace_root: &Path,
) -> Result<Vec<PathBuf>, KnowerageError> {
    if !selection.paths.is_empty() {
        let mut seen = std::collections::HashSet::new();
        return Ok(selection
            .paths
            .iter()
            .filter(|p| seen.insert(*p))
            .cloned()
            .collect());
    }

    let records = registry.load()?;
    let mut entries: Vec<_> = records.values().collect();
    entries.sort_by(|a, b| b.record_updated_at.cmp(&a.record_updated_at));

    let paths: Vec<PathBuf> = entries.iter().map(|r| r.analysis_path.clone()).collect();

    Ok(match selection.limit {
        Some(limit) => paths.into_iter().take(limit).collect(),
        None => paths,
    })
}

fn path_for_validation(path: &Path) -> Cow<'_, str> {
    path.to_string_lossy()
}

fn workspace_relative_display(workspace_root: &Path, validated_abs: &Path) -> PathBuf {
    let Ok(canonical_root) = fs::canonicalize(workspace_root) else {
        return validated_abs.to_path_buf();
    };
    let Ok(canonical_file) = fs::canonicalize(validated_abs) else {
        return validated_abs.to_path_buf();
    };
    canonical_file
        .strip_prefix(&canonical_root)
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| PathBuf::from(validated_abs.file_name().unwrap_or_default()))
}

fn toc_table_header() -> &'static str {
    "| # | Analysis | Source | Ranges |\n|---|----------|--------|--------|\n"
}

fn build_toc(rows: &[String]) -> String {
    format!("{}{}", toc_table_header(), rows.join("\n"))
}

struct ActivePart {
    toc_rows: Vec<String>,
    combined_parts: Vec<String>,
    byte_size: usize,
    row_counter: usize,
    analysis_paths: Vec<PathBuf>,
}

impl ActivePart {
    fn new() -> Self {
        Self {
            toc_rows: Vec::new(),
            combined_parts: Vec::new(),
            byte_size: 0,
            row_counter: 0,
            analysis_paths: Vec::new(),
        }
    }

    fn flush(self) -> (ExportBundlePart, Vec<PathBuf>, usize) {
        let combined = self.combined_parts.join(COMBINED_SEPARATOR);
        let byte_len = combined.len();
        let toc = build_toc(&self.toc_rows);
        let paths = self.analysis_paths;
        (
            ExportBundlePart { toc, combined },
            paths,
            byte_len,
        )
    }

    fn added_bytes_if_append(&self, content_len: usize) -> usize {
        if self.combined_parts.is_empty() {
            content_len
        } else {
            COMBINED_SEPARATOR.len() + content_len
        }
    }

    fn append(
        &mut self,
        display_path: PathBuf,
        source_display: PathBuf,
        ranges: String,
        content: String,
    ) {
        self.row_counter += 1;
        self.toc_rows.push(format!(
            "| {} | {} | {} | {} |",
            self.row_counter,
            display_path.display(),
            source_display.display(),
            ranges,
        ));
        self.combined_parts.push(content);
        self.byte_size = self.combined_parts.join(COMBINED_SEPARATOR).len();
        self.analysis_paths.push(display_path);
    }
}

/// Build an export bundle from analysis paths (validated under `workspace_root`), with chunking
/// and per-file / per-part size limits. Fails if `paths.len()` exceeds [`MAX_ANALYSIS_PATHS`].
pub fn generate_bundle(
    paths: &[PathBuf],
    workspace_root: &Path,
) -> Result<ExportBundle, KnowerageError> {
    if paths.len() > MAX_ANALYSIS_PATHS {
        return Err(KnowerageError::DocParse(format!(
            "Too many analysis paths: {} (max {}). Split into multiple bundle exports.",
            paths.len(),
            MAX_ANALYSIS_PATHS
        )));
    }

    let mut errors = Vec::new();
    let mut file_entries = Vec::new();
    let mut sealed: Vec<(ExportBundlePart, Vec<PathBuf>, usize)> = Vec::new();

    let mut current_part_num: u32 = 1;
    let mut active = ActivePart::new();

    'paths: for path in paths {
        let path_str = path_for_validation(path);
        let validated_abs = match security::validate_path(workspace_root, path_str.as_ref()) {
            Ok(p) => p,
            Err(e) => {
                errors.push(ExportError {
                    path: path.clone(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        let display_rel = workspace_relative_display(workspace_root, &validated_abs);

        let content = match fs::read_to_string(&validated_abs) {
            Ok(c) => c,
            Err(e) => {
                errors.push(ExportError {
                    path: path.clone(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        if content.len() > MAX_BYTES_PER_ANALYSIS_FILE {
            errors.push(ExportError {
                path: path.clone(),
                error: format!(
                    "Analysis file exceeds max size ({} bytes; max {})",
                    content.len(),
                    MAX_BYTES_PER_ANALYSIS_FILE
                ),
            });
            continue;
        }

        let metadata = match parse_frontmatter(&content) {
            Ok(m) => m,
            Err(e) => {
                errors.push(ExportError {
                    path: path.clone(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        let hash = {
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            format!("sha256:{:x}", hasher.finalize())
        };

        let ranges: String = metadata
            .covered_lines
            .iter()
            .map(|r| format!("{}-{}", r[0], r[1]))
            .collect::<Vec<_>>()
            .join(", ");

        let source_display = metadata.source_file.clone();

        loop {
            let added = active.added_bytes_if_append(content.len());

            if content.len() > MAX_COMBINED_PART_BYTES {
                errors.push(ExportError {
                    path: path.clone(),
                    error: format!(
                        "Single analysis body exceeds max part size ({} bytes; max {})",
                        content.len(),
                        MAX_COMBINED_PART_BYTES
                    ),
                });
                continue 'paths;
            }

            if !active.combined_parts.is_empty()
                && active.byte_size.saturating_add(added) > MAX_COMBINED_PART_BYTES
            {
                let flushed = std::mem::replace(&mut active, ActivePart::new());
                sealed.push(flushed.flush());
                current_part_num = current_part_num.saturating_add(1);
                continue;
            }

            active.append(
                display_rel.clone(),
                source_display,
                ranges,
                content.clone(),
            );
            file_entries.push(ExportFileEntry {
                analysis_path: display_rel,
                source_path: metadata.source_file,
                content_hash: hash,
                part_index: current_part_num,
            });
            continue 'paths;
        }
    }

    if !active.combined_parts.is_empty() {
        sealed.push(active.flush());
    }
    if sealed.is_empty() {
        sealed.push((
            ExportBundlePart {
                toc: build_toc(&[]),
                combined: String::new(),
            },
            vec![],
            0,
        ));
    }

    let mut bundle_parts = Vec::new();
    let mut manifest_parts = Vec::new();

    for (pi, (part, analysis_paths, byte_len)) in sealed.into_iter().enumerate() {
        let idx = (pi + 1) as u32;
        let (toc_file, combined_file) = if idx == 1 {
            ("toc.md".to_string(), "combined.md".to_string())
        } else {
            (format!("toc_{idx}.md"), format!("combined_{idx}.md"))
        };
        manifest_parts.push(ExportPartInfo {
            part_index: idx,
            toc_file: toc_file.clone(),
            combined_file: combined_file.clone(),
            combined_byte_length: byte_len,
            analysis_paths,
        });
        bundle_parts.push(part);
    }

    Ok(ExportBundle {
        parts: bundle_parts,
        manifest: ExportManifest {
            created_at: Utc::now(),
            files: file_entries,
            errors,
            parts: manifest_parts,
        },
    })
}

/// Writes all bundle parts and `manifest.json` under `output_dir`. Returns relative filenames
/// (no directory prefix) in write order.
pub fn write_bundle(bundle: &ExportBundle, output_dir: &Path) -> Result<Vec<String>, KnowerageError> {
    fs::create_dir_all(output_dir)
        .map_err(|e| KnowerageError::RegistryIo(format!("Failed to create output dir: {e}")))?;

    let mut written = Vec::new();

    for (pi, part) in bundle.parts.iter().enumerate() {
        let idx = (pi + 1) as u32;
        let toc_name = if idx == 1 {
            "toc.md".to_string()
        } else {
            format!("toc_{idx}.md")
        };
        let comb_name = if idx == 1 {
            "combined.md".to_string()
        } else {
            format!("combined_{idx}.md")
        };
        security::atomic_write(&output_dir.join(&toc_name), part.toc.as_bytes())?;
        written.push(toc_name);
        security::atomic_write(&output_dir.join(&comb_name), part.combined.as_bytes())?;
        written.push(comb_name);
    }

    let manifest_json = serde_json::to_string_pretty(&bundle.manifest)
        .map_err(|e| KnowerageError::RegistryIo(format!("Failed to serialize manifest: {e}")))?;
    security::atomic_write(&output_dir.join("manifest.json"), manifest_json.as_bytes())?;
    written.push("manifest.json".to_string());

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn valid_frontmatter(source: &str) -> String {
        format!(
            "---\nsource_file: \"{source}\"\ncovered_lines:\n  - [1, 50]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n# Analysis\n"
        )
    }

    #[test]
    fn test_selection_with_limit() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();

        fs::create_dir_all(ws.join("knowerage/analysis")).unwrap();
        fs::create_dir_all(ws.join("src")).unwrap();

        for name in &["a", "b", "c", "d"] {
            let src_rel = format!("src/{name}.java");
            fs::write(ws.join(&src_rel), format!("class {name} {{}}")).unwrap();
            let analysis_rel = format!("knowerage/analysis/{name}.md");
            fs::write(ws.join(&analysis_rel), valid_frontmatter(&src_rel)).unwrap();
        }

        let registry = Registry::new(ws.to_path_buf());
        for name in &["a", "b", "c", "d"] {
            let analysis_rel = PathBuf::from(format!("knowerage/analysis/{name}.md"));
            let content = fs::read_to_string(ws.join(&analysis_rel)).unwrap();
            let metadata = parse_frontmatter(&content).unwrap();
            registry.reconcile_record(&analysis_rel, &metadata).unwrap();
        }

        let selection = ExportSelection {
            paths: vec![],
            limit: Some(2),
        };
        let result = select_files(&selection, &registry, ws).unwrap();
        assert!(result.len() <= 2);
    }

    #[test]
    fn test_duplicate_paths_deduped() {
        let tmp = TempDir::new().unwrap();
        let registry = Registry::new(tmp.path().to_path_buf());
        let selection = ExportSelection {
            paths: vec![
                PathBuf::from("a.md"),
                PathBuf::from("a.md"),
                PathBuf::from("b.md"),
            ],
            limit: None,
        };
        let result = select_files(&selection, &registry, tmp.path()).unwrap();
        assert_eq!(result, vec![PathBuf::from("a.md"), PathBuf::from("b.md")]);
    }

    #[test]
    fn test_manifest_contains_all_files() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();

        let paths: Vec<PathBuf> = (1..=3)
            .map(|i| {
                let name = format!("analysis_{i}.md");
                let src = format!("src/file_{i}.java");
                fs::write(ws.join(&name), valid_frontmatter(&src)).unwrap();
                PathBuf::from(name)
            })
            .collect();

        let bundle = generate_bundle(&paths, ws).unwrap();
        assert_eq!(bundle.manifest.files.len(), 3);
        for entry in &bundle.manifest.files {
            assert!(entry.content_hash.starts_with("sha256:"));
            assert!(entry.content_hash.len() > 10);
        }
    }

    #[test]
    fn test_manifest_timestamp_valid() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();

        fs::write(ws.join("ts_test.md"), valid_frontmatter("src/ts.java")).unwrap();

        let before = Utc::now();
        let bundle = generate_bundle(&[PathBuf::from("ts_test.md")], ws).unwrap();
        let after = Utc::now();

        assert!(bundle.manifest.created_at >= before);
        assert!(bundle.manifest.created_at <= after);
    }

    #[test]
    fn test_invalid_file_partial_success() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();

        fs::write(ws.join("valid.md"), valid_frontmatter("src/ok.java")).unwrap();

        let paths = vec![PathBuf::from("valid.md"), PathBuf::from("nonexistent.md")];
        let bundle = generate_bundle(&paths, ws).unwrap();

        assert_eq!(bundle.manifest.files.len(), 1);
        assert_eq!(bundle.manifest.errors.len(), 1);
        assert_eq!(
            bundle.manifest.errors[0].path,
            PathBuf::from("nonexistent.md")
        );
    }

    #[test]
    fn test_empty_selection() {
        let tmp = TempDir::new().unwrap();
        let bundle = generate_bundle(&[], tmp.path()).unwrap();

        assert!(bundle.manifest.files.is_empty());
        assert!(bundle.manifest.errors.is_empty());
        assert!(bundle.primary_combined().is_empty());
        assert_eq!(bundle.parts.len(), 1);
    }

    #[test]
    fn test_path_traversal_rejected() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("knowerage/analysis")).unwrap();
        fs::write(
            ws.join("knowerage/analysis/safe.md"),
            valid_frontmatter("src/x.java"),
        )
        .unwrap();

        let paths = vec![
            PathBuf::from("knowerage/analysis/safe.md"),
            PathBuf::from("../outside.md"),
        ];
        let bundle = generate_bundle(&paths, ws).unwrap();
        assert_eq!(bundle.manifest.files.len(), 1);
        assert_eq!(bundle.manifest.errors.len(), 1);
        assert!(
            bundle.manifest.errors[0]
                .error
                .contains("E_PATH_TRAVERSAL") || bundle.manifest.errors[0].error.contains("..")
        );
    }

    #[test]
    fn test_chunking_two_parts() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();

        // First part nearly fills `MAX_COMBINED_PART_BYTES`; second doc cannot fit with separator.
        let fm = valid_frontmatter("src/a.java");
        let pad_len = MAX_COMBINED_PART_BYTES
            .saturating_sub(fm.len())
            .saturating_sub("# A\n".len())
            .saturating_sub(1);
        let mut doc_a = fm;
        doc_a.push_str("# A\n");
        doc_a.push_str(&"x".repeat(pad_len));
        doc_a.push('\n');

        let mut doc_b = valid_frontmatter("src/b.java");
        doc_b.push_str("# B\nmore\n");

        fs::write(ws.join("a.md"), &doc_a).unwrap();
        fs::write(ws.join("b.md"), &doc_b).unwrap();

        assert_eq!(
            fs::read(ws.join("a.md")).unwrap().len(),
            MAX_COMBINED_PART_BYTES,
            "on-disk a.md should match in-memory size"
        );
        assert_eq!(
            doc_a.len(),
            MAX_COMBINED_PART_BYTES,
            "fixture a should be exactly max part bytes"
        );
        assert!(
            doc_a.len() + doc_b.len() + COMBINED_SEPARATOR.len() > MAX_COMBINED_PART_BYTES,
            "a+b should not fit in one part"
        );

        let bundle = generate_bundle(&[PathBuf::from("a.md"), PathBuf::from("b.md")], ws).unwrap();
        assert_eq!(
            bundle.parts.len(),
            2,
            "expected two combined parts, got {}",
            bundle.parts.len()
        );
        assert_eq!(bundle.manifest.files.len(), 2);
        assert_eq!(bundle.manifest.parts.len(), 2);
        assert_eq!(bundle.manifest.files[0].part_index, 1);
        assert_eq!(bundle.manifest.files[1].part_index, 2);
    }

    #[test]
    fn test_oversized_single_file_error() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();
        let huge = "z".repeat(MAX_BYTES_PER_ANALYSIS_FILE + 1);
        let mut doc = valid_frontmatter("src/h.java");
        doc.push_str(&huge);
        fs::write(ws.join("big.md"), doc).unwrap();

        let bundle = generate_bundle(&[PathBuf::from("big.md")], ws).unwrap();
        assert!(bundle.manifest.files.is_empty());
        assert_eq!(bundle.manifest.errors.len(), 1);
        assert!(bundle
            .manifest
            .errors[0]
            .error
            .contains("exceeds max size"));
    }

    #[test]
    fn test_too_many_paths_rejected() {
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path();
        let paths: Vec<PathBuf> = (0..MAX_ANALYSIS_PATHS + 1)
            .map(|i| PathBuf::from(format!("f{i}.md")))
            .collect();
        let err = match generate_bundle(&paths, ws) {
            Err(e) => e,
            Ok(_) => panic!("expected Too many analysis paths error"),
        };
        assert!(err.to_string().contains("Too many analysis paths"));
    }
}
