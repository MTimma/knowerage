use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::export;
use crate::parser;
use crate::project_scan;
use crate::registry::Registry;
use crate::security::{self, RegistryLock};
use crate::types::*;

#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

pub struct McpServer {
    workspace_root: PathBuf,
    registry: Registry,
}

impl McpServer {
    pub fn new(workspace_root: PathBuf) -> Self {
        let registry = Registry::new(workspace_root.clone());
        Self {
            workspace_root,
            registry,
        }
    }

    pub fn new_with_lock(workspace_root: PathBuf, registry_lock: Arc<RegistryLock>) -> Self {
        let registry = Registry::with_lock(workspace_root.clone(), registry_lock);
        Self {
            workspace_root,
            registry,
        }
    }

    fn to_relative(&self, abs_path: &Path) -> Result<PathBuf, KnowerageError> {
        let canonical_root = fs::canonicalize(&self.workspace_root).map_err(|e| {
            KnowerageError::PathTraversal(format!("Cannot canonicalize workspace root: {e}"))
        })?;
        abs_path
            .strip_prefix(&canonical_root)
            .map(|p| p.to_path_buf())
            .map_err(|_| {
                KnowerageError::PathTraversal("Resolved path is outside workspace root".into())
            })
    }

    /// Validates a path for file creation: rejects traversal / absolute paths,
    /// creates parent directories, then delegates to `security::validate_path`.
    /// Returns `(absolute_path, relative_path)`.
    fn ensure_parent_and_validate(
        &self,
        input: &str,
    ) -> Result<(PathBuf, PathBuf), KnowerageError> {
        for segment in input.split(['/', '\\']) {
            if segment == ".." {
                return Err(KnowerageError::PathTraversal(
                    "Path contains '..' segment".into(),
                ));
            }
        }
        if Path::new(input).is_absolute() {
            return Err(KnowerageError::PathTraversal(
                "Absolute paths not allowed for file creation".into(),
            ));
        }

        let target = self.workspace_root.join(input);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                KnowerageError::RegistryIo(format!("Failed to create directory: {e}"))
            })?;
        }

        let abs_path = security::validate_path(&self.workspace_root, input)?;
        let rel_path = self.to_relative(&abs_path)?;
        Ok((abs_path, rel_path))
    }

    pub fn dispatch_tool(&self, name: &str, args: Value) -> Result<Value, KnowerageError> {
        match name {
            "knowerage.create_or_update_doc" => self.handle_create_or_update_doc(args),
            "knowerage.parse_doc_metadata" => self.handle_parse_doc_metadata(args),
            "knowerage.reconcile_record" => self.handle_reconcile_record(args),
            "knowerage.reconcile_all" => self.handle_reconcile_all(args),
            "knowerage.get_file_status" => self.handle_get_file_status(args),
            "knowerage.list_stale" => self.handle_list_stale(args),
            "knowerage.list_registry" => self.handle_list_registry(args),
            "knowerage.get_tree" => self.handle_get_tree(args),
            "knowerage.coverage_overview" => self.handle_coverage_overview(args),
            "registry.export_report" => self.handle_export_report(args),
            "knowerage.generate_bundle" => self.handle_generate_bundle(args),
            _ => Err(KnowerageError::DocParse(format!("Unknown tool: {name}"))),
        }
    }

    // ── Tool 1: create_or_update_doc ────────────────────────────────────

    fn handle_create_or_update_doc(&self, args: Value) -> Result<Value, KnowerageError> {
        let analysis_path_str = require_str(&args, "analysis_path")?;
        let source_path_str = require_str(&args, "source_path")?;
        let content_body = require_str(&args, "content")?;

        if security::looks_like_secret(content_body) {
            return Err(KnowerageError::DocParse(
                "Content body appears to contain secrets or credentials".into(),
            ));
        }

        let covered_lines = parse_covered_lines(&args)?;

        security::validate_path(&self.workspace_root, source_path_str)?;
        let (abs_analysis, rel_analysis) = self.ensure_parent_and_validate(analysis_path_str)?;

        let sanitized = security::sanitize_string(content_body, 1_000_000);
        let now = Utc::now();
        let frontmatter = build_frontmatter(source_path_str, &covered_lines, &now);
        let full = format!("{frontmatter}{sanitized}\n");

        security::atomic_write(&abs_analysis, full.as_bytes())?;

        Ok(serde_json::json!({
            "ok": true,
            "analysis_path": rel_analysis.display().to_string()
        }))
    }

    // ── Tool 2: parse_doc_metadata ──────────────────────────────────────

    fn handle_parse_doc_metadata(&self, args: Value) -> Result<Value, KnowerageError> {
        let analysis_path_str = require_str(&args, "analysis_path")?;
        let abs_path = security::validate_path(&self.workspace_root, analysis_path_str)?;

        let content = fs::read_to_string(&abs_path)
            .map_err(|e| KnowerageError::DocParse(format!("Cannot read analysis file: {e}")))?;

        let metadata = parser::parse_frontmatter(&content)?;
        serde_json::to_value(&metadata)
            .map_err(|e| KnowerageError::RegistryIo(format!("Serialization error: {e}")))
    }

    // ── Tool 3: reconcile_record ────────────────────────────────────────

    fn handle_reconcile_record(&self, args: Value) -> Result<Value, KnowerageError> {
        let analysis_path_str = require_str(&args, "analysis_path")?;
        let abs_path = security::validate_path(&self.workspace_root, analysis_path_str)?;
        let rel_path = self.to_relative(&abs_path)?;

        let content = fs::read_to_string(&abs_path)
            .map_err(|e| KnowerageError::DocParse(format!("Cannot read analysis file: {e}")))?;

        let metadata = parser::parse_frontmatter(&content)?;
        let record = self.registry.reconcile_record(&rel_path, &metadata)?;

        serde_json::to_value(&record)
            .map_err(|e| KnowerageError::RegistryIo(format!("Serialization error: {e}")))
    }

    // ── Tool 4: reconcile_all ───────────────────────────────────────────

    fn handle_reconcile_all(&self, _args: Value) -> Result<Value, KnowerageError> {
        let summary = self.registry.reconcile_all()?;
        serde_json::to_value(&summary)
            .map_err(|e| KnowerageError::RegistryIo(format!("Serialization error: {e}")))
    }

    // ── Tool 5: get_file_status ─────────────────────────────────────────

    fn handle_get_file_status(&self, args: Value) -> Result<Value, KnowerageError> {
        let source_path_str = require_str(&args, "source_path")?;
        let abs_source = security::validate_path(&self.workspace_root, source_path_str)?;
        let rel_source = self.to_relative(&abs_source)?;

        let content = fs::read_to_string(&abs_source)
            .map_err(|e| KnowerageError::SrcMissing(format!("Cannot read source file: {e}")))?;
        let total_lines = content.lines().count() as u64;

        let records = self.registry.load()?;
        let mut all_ranges: Vec<[u64; 2]> = Vec::new();
        let mut attributions: Vec<RangeAttribution> = Vec::new();

        for record in records.values() {
            if record.source_path == rel_source {
                for range in &record.covered_ranges {
                    all_ranges.push(*range);
                    attributions.push(RangeAttribution {
                        range: *range,
                        analysis_path: record.analysis_path.clone(),
                    });
                }
            }
        }

        let analyzed_ranges = parser::normalize_ranges(&all_ranges);
        let missing_ranges = compute_missing_ranges(&analyzed_ranges, total_lines);

        let covered_count: u64 = analyzed_ranges.iter().map(|r| r[1] - r[0] + 1).sum();
        let coverage_percent = if total_lines > 0 {
            (covered_count as f64 / total_lines as f64) * 100.0
        } else {
            0.0
        };

        let status = FileStatus {
            source_path: rel_source,
            total_lines,
            analyzed_ranges,
            missing_ranges,
            coverage_percent,
            range_attribution: attributions,
        };

        serde_json::to_value(&status)
            .map_err(|e| KnowerageError::RegistryIo(format!("Serialization error: {e}")))
    }

    // ── Tool 6: list_stale ──────────────────────────────────────────────

    fn handle_list_stale(&self, args: Value) -> Result<Value, KnowerageError> {
        let status_filter: Option<Vec<String>> = args
            .get("statuses")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let records = self.registry.load()?;
        let filtered: Vec<&RegistryRecord> = records
            .values()
            .filter(|r| match &status_filter {
                Some(statuses) => statuses.iter().any(|s| r.status.to_string() == *s),
                None => r.status != FreshnessStatus::Fresh,
            })
            .collect();

        serde_json::to_value(&filtered)
            .map_err(|e| KnowerageError::RegistryIo(format!("Serialization error: {e}")))
    }

    // ── Tool 6b: list_registry ─────────────────────────────────────────

    /// Full registry snapshot for agents: same record shape as `knowerage/registry.json`, keys sorted.
    fn handle_list_registry(&self, args: Value) -> Result<Value, KnowerageError> {
        let prefix = args
            .get("analysis_path_prefix")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let status_filter: Option<Vec<String>> = args
            .get("statuses")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let records = self.registry.load()?;
        let mut out: BTreeMap<String, RegistryRecord> = BTreeMap::new();

        for (key, record) in records {
            if !prefix.is_empty() && !key.starts_with(prefix) {
                continue;
            }
            if let Some(ref statuses) = status_filter {
                if !statuses
                    .iter()
                    .any(|s| record.status.to_string() == *s)
                {
                    continue;
                }
            }
            out.insert(key, record);
        }

        let record_count = out.len();
        Ok(serde_json::json!({
            "schema_note": "The \"records\" object matches the root JSON object of knowerage/registry.json (keys = analysis markdown paths relative to workspace root; values = full registry rows: source_path, covered_ranges, status, hashes, timestamps). Prefer this tool over reading the file so you get validated, sorted output. Do not hand-edit registry.json.",
            "registry_file": "knowerage/registry.json",
            "record_count": record_count,
            "records": out,
        }))
    }

    // ── Tool 7: get_tree ────────────────────────────────────────────────

    fn handle_get_tree(&self, args: Value) -> Result<Value, KnowerageError> {
        let root = args.get("root").and_then(|v| v.as_str()).unwrap_or("");

        let records = self.registry.load()?;
        let mut groups: BTreeMap<String, Vec<RegistryRecord>> = BTreeMap::new();

        for record in records.into_values() {
            let source_str = record.source_path.to_string_lossy().to_string();
            if !root.is_empty() && !source_str.starts_with(root) {
                continue;
            }
            let dir = record
                .source_path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            groups.entry(dir).or_default().push(record);
        }

        let tree: Vec<Value> = groups
            .into_iter()
            .map(|(dir, recs)| {
                serde_json::json!({
                    "directory": dir,
                    "record_count": recs.len(),
                    "records": recs,
                })
            })
            .collect();

        Ok(Value::Array(tree))
    }

    // ── Tool 8: coverage_overview ─────────────────────────────────────

    fn handle_coverage_overview(&self, args: Value) -> Result<Value, KnowerageError> {
        let extensions = parse_extensions_arg(&args)?;
        let records = self.registry.load()?;

        let mut source_groups: BTreeMap<String, Vec<&RegistryRecord>> = BTreeMap::new();
        let mut stale_records: Vec<&RegistryRecord> = Vec::new();

        for record in records.values() {
            let key = record.source_path.to_string_lossy().to_string();
            source_groups.entry(key).or_default().push(record);
            if record.status != FreshnessStatus::Fresh {
                stale_records.push(record);
            }
        }

        let mut sources: Vec<Value> = Vec::new();
        let mut total_covered: u64 = 0;
        let mut total_lines_sum: u64 = 0;
        let mut missing_src_count: u64 = 0;

        for (src_key, recs) in &source_groups {
            if !source_path_matches_extensions(src_key, &extensions) {
                continue;
            }

            let total_lines = match security::validate_path(
                &self.workspace_root,
                &recs[0].source_path.to_string_lossy(),
            )
            .and_then(|abs| {
                fs::read_to_string(&abs)
                    .map_err(|e| KnowerageError::SrcMissing(format!("{e}")))
            }) {
                Ok(content) => content.lines().count() as u64,
                Err(_) => {
                    missing_src_count += 1;
                    0
                }
            };

            let mut all_ranges: Vec<[u64; 2]> = Vec::new();
            let mut attributions: Vec<Value> = Vec::new();

            for record in recs {
                if record.status == FreshnessStatus::Fresh {
                    for range in &record.covered_ranges {
                        all_ranges.push(*range);
                        attributions.push(serde_json::json!({
                            "range": range,
                            "analysis_path": record.analysis_path.to_string_lossy()
                        }));
                    }
                }
            }

            let analyzed_ranges = parser::normalize_ranges(&all_ranges);
            let missing_ranges = compute_missing_ranges(&analyzed_ranges, total_lines);

            let covered_count: u64 = analyzed_ranges.iter().map(|r| r[1] - r[0] + 1).sum();
            let coverage_percent = if total_lines > 0 {
                (covered_count as f64 / total_lines as f64) * 100.0
            } else {
                0.0
            };

            total_covered += covered_count;
            total_lines_sum += total_lines;

            sources.push(serde_json::json!({
                "source_path": src_key,
                "total_lines": total_lines,
                "analyzed_ranges": analyzed_ranges,
                "missing_ranges": missing_ranges,
                "coverage_percent": (coverage_percent * 100.0).round() / 100.0,
                "range_attribution": attributions,
            }));
        }

        let avg_coverage_pct = if total_lines_sum > 0 {
            (total_covered as f64 / total_lines_sum as f64) * 100.0
        } else {
            0.0
        };

        let (project_files, project_lines) =
            project_scan::project_files_and_lines(&self.workspace_root, &extensions)?;

        let tracked_files = sources.len() as u64;
        let covered_lines = total_covered;
        let tracked_lines = total_lines_sum;

        let pct_files_tracked = if project_files > 0 {
            (tracked_files as f64 / project_files as f64) * 100.0
        } else {
            0.0
        };
        let pct_lines_covered_vs_project = if project_lines > 0 {
            (covered_lines as f64 / project_lines as f64) * 100.0
        } else {
            0.0
        };

        let stale_for_output: Vec<&RegistryRecord> = stale_records
            .into_iter()
            .filter(|r| {
                source_path_matches_extensions(&r.source_path.to_string_lossy(), &extensions)
            })
            .collect();

        Ok(serde_json::json!({
            "summary": {
                "total_sources": tracked_files,
                "tracked_files": tracked_files,
                "total_records": records.len(),
                "avg_coverage_pct": (avg_coverage_pct * 100.0).round() / 100.0,
                "stale_count": stale_for_output.len(),
                "missing_src_count": missing_src_count,
                "project_files": project_files,
                "project_lines": project_lines,
                "tracked_lines": tracked_lines,
                "covered_lines": covered_lines,
                "pct_files_tracked": (pct_files_tracked * 100.0).round() / 100.0,
                "pct_lines_covered_vs_project": (pct_lines_covered_vs_project * 100.0).round() / 100.0,
                "extensions_applied": extensions,
            },
            "sources": sources,
            "stale_records": stale_for_output,
        }))
    }

    // ── Tool 10: export_report ──────────────────────────────────────────

    fn handle_export_report(&self, args: Value) -> Result<Value, KnowerageError> {
        let format_str = require_str(&args, "format")?;
        let output_path_str = require_str(&args, "output_path")?;

        let (abs_output, rel_output) = self.ensure_parent_and_validate(output_path_str)?;
        let records = self.registry.load()?;

        export::generate_report(&records, format_str, &abs_output)?;

        Ok(serde_json::json!({
            "ok": true,
            "output_path": rel_output.display().to_string()
        }))
    }

    // ── Tool 11: generate_bundle ────────────────────────────────────────

    fn handle_generate_bundle(&self, args: Value) -> Result<Value, KnowerageError> {
        let output_dir_str = require_str(&args, "output_dir")?;
        let (abs_output_dir, rel_output_dir) = self.ensure_parent_and_validate(output_dir_str)?;

        let path_strs = require_string_array(&args, "analysis_paths")?;
        if path_strs.len() > crate::export::MAX_ANALYSIS_PATHS {
            return Err(KnowerageError::DocParse(format!(
                "Too many analysis_paths: {} (max {}). Split into multiple generate_bundle calls.",
                path_strs.len(),
                crate::export::MAX_ANALYSIS_PATHS
            )));
        }

        let mut path_bufs: Vec<PathBuf> = Vec::with_capacity(path_strs.len());
        for p in &path_strs {
            security::validate_path(&self.workspace_root, p)?;
            path_bufs.push(PathBuf::from(*p));
        }

        let bundle = export::generate_bundle(&path_bufs, &self.workspace_root)?;
        let files_written = export::write_bundle(&bundle, &abs_output_dir)?;

        let manifest_json = serde_json::to_value(&bundle.manifest).map_err(|e| {
            KnowerageError::RegistryIo(format!("Failed to serialize manifest: {e}"))
        })?;

        Ok(serde_json::json!({
            "ok": true,
            "output_dir": rel_output_dir.display().to_string(),
            "files_written": files_written,
            "manifest": manifest_json,
        }))
    }

    // ── MCP protocol ────────────────────────────────────────────────────

    fn tool_definitions() -> Vec<Value> {
        vec![
            serde_json::json!({
                "name": "knowerage.create_or_update_doc",
                "description": "PRIMARY tool for persisting legacy/source code analysis: create or update an analysis markdown file under knowerage/analysis/ with YAML frontmatter (source path, covered line ranges, dates). Use this whenever the user asks to analyze, document, or explain a source file and Knowerage is in the tool list—do not use generic file-write tools alone for knowerage analysis paths, or the registry will not stay consistent. Follow with knowerage.reconcile_record on the same analysis_path.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "analysis_path": { "type": "string", "description": "Relative path for the analysis markdown file" },
                        "source_path": { "type": "string", "description": "Relative path to the source file being analyzed" },
                        "covered_lines": {
                            "type": "array",
                            "items": { "type": "array", "items": { "type": "integer" }, "minItems": 2, "maxItems": 2 },
                            "description": "Array of [start, end] line ranges"
                        },
                        "content": { "type": "string", "description": "Markdown body content for the analysis" }
                    },
                    "required": ["analysis_path", "source_path", "covered_lines", "content"]
                }
            }),
            serde_json::json!({
                "name": "knowerage.parse_doc_metadata",
                "description": "Parse YAML frontmatter from an analysis markdown file",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "analysis_path": { "type": "string", "description": "Relative path to the analysis file" }
                    },
                    "required": ["analysis_path"]
                }
            }),
            serde_json::json!({
                "name": "knowerage.reconcile_record",
                "description": "MANDATORY after every create_or_update_doc (or manual edit) to one analysis file: reconciles that analysis into knowerage/registry.json (hashes, covered_ranges, freshness). Call immediately after writing analysis content so coverage is recorded; skipping this leaves the registry wrong or stale.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "analysis_path": { "type": "string", "description": "Relative path to the analysis file" }
                    },
                    "required": ["analysis_path"]
                }
            }),
            serde_json::json!({
                "name": "knowerage.reconcile_all",
                "description": "Rescan all analysis markdown files matching the glob and rebuild registry entries. Use after git pull, bulk edits, or when the registry may be empty or out of date; prefer reconcile_record when only one file changed.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "analysis_glob": {
                            "type": "string",
                            "description": "Glob pattern for analysis files (default: knowerage/analysis/**/*.md)"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "knowerage.get_file_status",
                "description": "Per-source coverage: total lines, analyzed vs missing ranges, and which analysis paths claim them. Prefer this over guessing from open files when answering 'what is documented for this source?' while Knowerage is enabled.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source_path": { "type": "string", "description": "Relative path to the source file" }
                    },
                    "required": ["source_path"]
                }
            }),
            serde_json::json!({
                "name": "knowerage.list_stale",
                "description": "List registry records filtered by staleness status",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "statuses": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": ["stale_doc", "stale_src", "missing_src", "dangling_doc"]
                            },
                            "description": "Filter by these statuses; omit to list all non-fresh"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "knowerage.list_registry",
                "description": "Return the full analysis coverage registry in one structured JSON response. USE THIS when you need an inventory of which source files are documented, where the analysis markdown lives, which line ranges are claimed, and freshness (fresh/stale_doc/stale_src/missing_src/dangling_doc). The `records` object is the same shape as the entire knowerage/registry.json file (sorted keys for stable reading). Call early when planning documentation work, finding gaps, or mapping analysis paths to sources—instead of opening registry.json by hand. Optional filters: analysis_path_prefix narrows by analysis key; statuses limits to given status values (same strings as in each record). After reconcile_record/reconcile_all, call again if you need the latest snapshot.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "analysis_path_prefix": {
                            "type": "string",
                            "description": "If non-empty, only include records whose analysis path key starts with this prefix (e.g. knowerage/analysis/auth/)"
                        },
                        "statuses": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": ["fresh", "stale_doc", "stale_src", "missing_src", "dangling_doc"]
                            },
                            "description": "If set, only include records whose status is one of these values; omit for all statuses"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "knowerage.get_tree",
                "description": "Get a tree view of analysis records grouped by directory",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root directory prefix to filter by", "default": "" },
                        "group_by": { "type": "string", "enum": ["directory"], "default": "directory" }
                    }
                }
            }),
            serde_json::json!({
                "name": "knowerage.coverage_overview",
                "description": "Batch coverage overview for all source files. Returns per-source coverage percentage, analyzed/missing ranges, range attribution, stale records, and project-wide file/line totals. Uses fresh records only for coverage calculation. Optional extensions filter applies to sources, stale list, and project scan.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "extensions": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "File extensions without leading dot (e.g. java, xml). Omit or pass [] to use defaults: java, xml, properties, gradle, kt, groovy, scala, kts."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "registry.export_report",
                "description": "Export the registry as a report file in json, yaml, txt, or html format",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "enum": ["json", "yaml", "txt", "html"],
                            "description": "Output format"
                        },
                        "output_path": { "type": "string", "description": "Relative path for the output file" }
                    },
                    "required": ["format", "output_path"]
                }
            }),
            serde_json::json!({
                "name": "knowerage.generate_bundle",
                "description": "Export selected analysis markdown files into chunked NotebookLM-style bundles under output_dir: writes toc.md + combined.md for part 1, toc_N.md + combined_N.md for further parts when size limits require splitting, plus manifest.json (files, errors, part metadata). Every analysis_paths entry is validated under the workspace (no traversal). Per-file and per-part size limits apply (see contracts).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "analysis_paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Relative paths to analysis .md files (workspace-rooted; order preserved)"
                        },
                        "output_dir": {
                            "type": "string",
                            "description": "Relative directory under workspace where toc*.md, combined*.md, and manifest.json are written"
                        }
                    },
                    "required": ["analysis_paths", "output_dir"]
                }
            }),
        ]
    }

    fn handle_jsonrpc(&self, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
        if request.jsonrpc != "2.0" {
            return Some(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32600,
                    message: "Invalid JSON-RPC version".into(),
                    data: None,
                }),
            });
        }

        match request.method.as_str() {
            "initialize" => Some(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "knowerage-mcp",
                        "version": "0.1.0"
                    },
                    "instructions": "Knowerage is enabled: treat legacy/source-code analysis and documentation tasks as Knowerage workflows. Do not create knowerage/analysis/*.md with plain file writes alone or hand-edit knowerage/registry.json. For each analysis: (1) knowerage.create_or_update_doc with source_path, covered_lines, and frontmatter-consistent content; (2) knowerage.reconcile_record on that analysis_path. For inventory/gaps use knowerage.list_registry, knowerage.get_file_status, knowerage.coverage_overview, or knowerage.list_stale instead of reading registry.json manually. After bulk source changes, knowerage.reconcile_all. For NotebookLM-style bulk export of selected analyses to markdown parts under a directory, use knowerage.generate_bundle (see contracts)."
                })),
                error: None,
            }),

            "notifications/initialized" => None,

            "tools/list" => Some(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: Some(serde_json::json!({ "tools": Self::tool_definitions() })),
                error: None,
            }),

            "tools/call" => {
                let params = request.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(Value::Object(serde_json::Map::new()));

                let result_value = match self.dispatch_tool(tool_name, arguments) {
                    Ok(val) => {
                        let text = serde_json::to_string(&val).unwrap_or_default();
                        serde_json::json!({
                            "content": [{ "type": "text", "text": text }]
                        })
                    }
                    Err(e) => {
                        serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("[{}] {}", e.code(), e)
                            }],
                            "isError": true
                        })
                    }
                };

                Some(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: request.id,
                    result: Some(result_value),
                    error: None,
                })
            }

            _ => {
                if request.id.is_some() {
                    Some(JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Method not found: {}", request.method),
                            data: None,
                        }),
                    })
                } else {
                    None
                }
            }
        }
    }

    pub fn run_stdio(&self) -> io::Result<()> {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        let stdout = io::stdout();
        let mut writer = io::BufWriter::new(stdout.lock());

        for line_result in reader.lines() {
            let line = line_result?;
            if line.trim().is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let err_response = JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {e}"),
                            data: None,
                        }),
                    };
                    let json = serde_json::to_string(&err_response).unwrap_or_else(|_| {
                        r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"}}"#
                            .into()
                    });
                    writeln!(writer, "{json}")?;
                    writer.flush()?;
                    continue;
                }
            };

            if let Some(response) = self.handle_jsonrpc(request) {
                let json = serde_json::to_string(&response).unwrap_or_else(|_| {
                    r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Internal error"}}"#.into()
                });
                writeln!(writer, "{json}")?;
                writer.flush()?;
            }
        }

        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn require_str<'a>(args: &'a Value, field: &str) -> Result<&'a str, KnowerageError> {
    args.get(field).and_then(|v| v.as_str()).ok_or_else(|| {
        KnowerageError::DocParse(format!("Missing or invalid required field: {field}"))
    })
}

fn require_string_array<'a>(args: &'a Value, field: &str) -> Result<Vec<&'a str>, KnowerageError> {
    let arr = args.get(field).and_then(|v| v.as_array()).ok_or_else(|| {
        KnowerageError::DocParse(format!(
            "Missing or invalid required field: {field} (expected array of strings)"
        ))
    })?;
    arr.iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_str().ok_or_else(|| {
                KnowerageError::DocParse(format!("{field}[{i}] must be a string"))
            })
        })
        .collect()
}

fn parse_covered_lines(args: &Value) -> Result<Vec<[u64; 2]>, KnowerageError> {
    let arr = args
        .get("covered_lines")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            KnowerageError::RangeInvalid(
                "Missing or invalid covered_lines: expected array of [start, end] pairs".into(),
            )
        })?;

    arr.iter()
        .map(|range| {
            let pair = range.as_array().ok_or_else(|| {
                KnowerageError::RangeInvalid("Each range must be a [start, end] array".into())
            })?;
            if pair.len() != 2 {
                return Err(KnowerageError::RangeInvalid(
                    "Each range must contain exactly 2 values".into(),
                ));
            }
            let start = pair[0].as_u64().ok_or_else(|| {
                KnowerageError::RangeInvalid("Range values must be positive integers".into())
            })?;
            let end = pair[1].as_u64().ok_or_else(|| {
                KnowerageError::RangeInvalid("Range values must be positive integers".into())
            })?;
            if start < 1 {
                return Err(KnowerageError::RangeInvalid(
                    "Range start must be >= 1".into(),
                ));
            }
            if end < start {
                return Err(KnowerageError::RangeInvalid(format!(
                    "Range end ({end}) must be >= start ({start})"
                )));
            }
            Ok([start, end])
        })
        .collect()
}

fn build_frontmatter(
    source_path: &str,
    covered_lines: &[[u64; 2]],
    analysis_date: &chrono::DateTime<Utc>,
) -> String {
    let mut fm = String::from("---\n");
    fm.push_str(&format!("source_file: \"{source_path}\"\n"));
    fm.push_str("covered_lines:\n");
    for range in covered_lines {
        fm.push_str(&format!("  - [{}, {}]\n", range[0], range[1]));
    }
    fm.push_str(&format!(
        "analysis_date: \"{}\"\n",
        analysis_date.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    fm.push_str("---\n");
    fm
}

fn parse_extensions_arg(args: &Value) -> Result<Vec<String>, KnowerageError> {
    match args.get("extensions") {
        None | Some(Value::Null) => Ok(project_scan::default_extensions()),
        Some(Value::Array(arr)) if arr.is_empty() => Ok(project_scan::default_extensions()),
        Some(Value::Array(arr)) => {
            let mut v = Vec::new();
            for item in arr {
                let s = item.as_str().ok_or_else(|| {
                    KnowerageError::DocParse("extensions[] entries must be strings".into())
                })?;
                v.push(s.to_string());
            }
            Ok(project_scan::normalize_extension_list(&v))
        }
        Some(_) => Err(KnowerageError::DocParse(
            "extensions must be an array of strings or omitted".into(),
        )),
    }
}

fn source_path_matches_extensions(source_path: &str, extensions: &[String]) -> bool {
    Path::new(source_path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| extensions.iter().any(|x| x.eq_ignore_ascii_case(ext)))
}

fn compute_missing_ranges(analyzed: &[[u64; 2]], total_lines: u64) -> Vec<[u64; 2]> {
    if total_lines == 0 {
        return vec![];
    }
    let mut missing = Vec::new();
    let mut current = 1u64;
    for range in analyzed {
        if current < range[0] {
            missing.push([current, range[0] - 1]);
        }
        current = range[1] + 1;
    }
    if current <= total_lines {
        missing.push([current, total_lines]);
    }
    missing
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_server(tmp: &TempDir) -> McpServer {
        let workspace_root = fs::canonicalize(tmp.path()).unwrap();
        fs::create_dir_all(workspace_root.join("knowerage/analysis")).unwrap();
        McpServer::new(workspace_root)
    }

    fn create_source_file(tmp: &TempDir, rel_path: &str, content: &str) {
        let workspace = fs::canonicalize(tmp.path()).unwrap();
        let abs_path = workspace.join(rel_path);
        if let Some(parent) = abs_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(abs_path, content).unwrap();
    }

    // Test 1: Unknown fields in request are handled gracefully
    #[test]
    fn test_unknown_fields_handled_gracefully() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);
        create_source_file(&tmp, "src/test.java", "class Test {}\n");

        let args = serde_json::json!({
            "analysis_path": "knowerage/analysis/test.md",
            "source_path": "src/test.java",
            "covered_lines": [[1, 1]],
            "content": "# Test",
            "unknown_field": "should be ignored",
            "extra_number": 42
        });

        let result = server.dispatch_tool("knowerage.create_or_update_doc", args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["ok"], true);
    }

    // Test 2: Path traversal with ".." is rejected
    #[test]
    fn test_path_traversal_rejected() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let args = serde_json::json!({
            "analysis_path": "../../etc/passwd",
            "source_path": "src/test.java",
            "covered_lines": [[1, 1]],
            "content": "# Test"
        });

        let err = server
            .dispatch_tool("knowerage.create_or_update_doc", args)
            .unwrap_err();
        assert_eq!(err.code(), "E_PATH_TRAVERSAL");
    }

    // Test 3: Absolute path outside workspace is rejected
    #[test]
    fn test_out_of_root_path_rejected() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let args = serde_json::json!({
            "analysis_path": "/etc/passwd",
            "source_path": "src/test.java",
            "covered_lines": [[1, 1]],
            "content": "# Test"
        });

        let err = server
            .dispatch_tool("knowerage.create_or_update_doc", args)
            .unwrap_err();
        assert_eq!(err.code(), "E_PATH_TRAVERSAL");
    }

    // Test 4: Invalid export format returns clear error
    #[test]
    fn test_export_format_validation() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let args = serde_json::json!({
            "format": "invalid",
            "output_path": "knowerage/report.xyz"
        });

        let err = server
            .dispatch_tool("registry.export_report", args)
            .unwrap_err();
        assert!(
            err.to_string().contains("Unsupported export format"),
            "Expected 'Unsupported export format' in: {err}"
        );
    }

    #[test]
    fn test_generate_bundle_writes_files() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);
        create_source_file(&tmp, "src/X.java", "class X {}\n");

        server
            .dispatch_tool(
                "knowerage.create_or_update_doc",
                serde_json::json!({
                    "analysis_path": "knowerage/analysis/x.md",
                    "source_path": "src/X.java",
                    "covered_lines": [[1, 1]],
                    "content": "# X analysis"
                }),
            )
            .unwrap();

        let out = server
            .dispatch_tool(
                "knowerage.generate_bundle",
                serde_json::json!({
                    "analysis_paths": ["knowerage/analysis/x.md"],
                    "output_dir": "knowerage/export/b1"
                }),
            )
            .unwrap();
        assert_eq!(out["ok"], true);
        let written = out["files_written"].as_array().unwrap();
        assert!(written.iter().any(|v| v == "toc.md"));
        assert!(written.iter().any(|v| v == "combined.md"));
        assert!(written.iter().any(|v| v == "manifest.json"));

        let workspace = fs::canonicalize(tmp.path()).unwrap();
        let mpath = workspace.join("knowerage/export/b1/manifest.json");
        let mf = fs::read_to_string(&mpath).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&mf).unwrap();
        assert_eq!(parsed["files"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_generate_bundle_rejects_traversal_in_analysis_paths() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let err = server
            .dispatch_tool(
                "knowerage.generate_bundle",
                serde_json::json!({
                    "analysis_paths": ["../../etc/passwd"],
                    "output_dir": "knowerage/out"
                }),
            )
            .unwrap_err();
        assert_eq!(err.code(), "E_PATH_TRAVERSAL");
    }

    // Test 5: create_or_update_doc creates file with valid frontmatter
    #[test]
    fn test_create_or_update_doc_success() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);
        create_source_file(&tmp, "src/App.java", "public class App {}\n");

        let args = serde_json::json!({
            "analysis_path": "knowerage/analysis/app.md",
            "source_path": "src/App.java",
            "covered_lines": [[1, 1]],
            "content": "# App Analysis\nThis analyzes App.java"
        });

        let result = server
            .dispatch_tool("knowerage.create_or_update_doc", args)
            .unwrap();
        assert_eq!(result["ok"], true);

        let workspace = fs::canonicalize(tmp.path()).unwrap();
        let created = fs::read_to_string(workspace.join("knowerage/analysis/app.md")).unwrap();
        assert!(created.contains("source_file:"));
        assert!(created.contains("src/App.java"));
        assert!(created.contains("covered_lines:"));
        assert!(created.contains("analysis_date:"));
        assert!(created.contains("# App Analysis"));
    }

    // Test 6: parse_doc_metadata returns parsed metadata
    #[test]
    fn test_parse_doc_metadata_success() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);
        create_source_file(&tmp, "src/App.java", "public class App {}\n");

        let create_args = serde_json::json!({
            "analysis_path": "knowerage/analysis/app.md",
            "source_path": "src/App.java",
            "covered_lines": [[1, 1]],
            "content": "# Analysis"
        });
        server
            .dispatch_tool("knowerage.create_or_update_doc", create_args)
            .unwrap();

        let parse_args = serde_json::json!({ "analysis_path": "knowerage/analysis/app.md" });
        let result = server
            .dispatch_tool("knowerage.parse_doc_metadata", parse_args)
            .unwrap();

        assert_eq!(result["source_file"], "src/App.java");
        assert!(!result["covered_lines"].as_array().unwrap().is_empty());
        assert!(result["analysis_date"].as_str().is_some());
    }

    // Test 7: reconcile_record returns status and hash
    #[test]
    fn test_reconcile_record_success() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);
        create_source_file(&tmp, "src/App.java", "public class App {}\n");

        let create_args = serde_json::json!({
            "analysis_path": "knowerage/analysis/app.md",
            "source_path": "src/App.java",
            "covered_lines": [[1, 1]],
            "content": "# Analysis"
        });
        server
            .dispatch_tool("knowerage.create_or_update_doc", create_args)
            .unwrap();

        let reconcile_args = serde_json::json!({ "analysis_path": "knowerage/analysis/app.md" });
        let result = server
            .dispatch_tool("knowerage.reconcile_record", reconcile_args)
            .unwrap();

        assert_eq!(result["status"], "fresh");
        assert!(result["analysis_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(result["source_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    }

    // Test 8: get_file_status returns analyzed/missing ranges and coverage
    #[test]
    fn test_get_file_status_success() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let source_content = (1..=10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        create_source_file(&tmp, "src/App.java", &source_content);

        let create_args = serde_json::json!({
            "analysis_path": "knowerage/analysis/app.md",
            "source_path": "src/App.java",
            "covered_lines": [[1, 5]],
            "content": "# Analysis"
        });
        server
            .dispatch_tool("knowerage.create_or_update_doc", create_args)
            .unwrap();

        let reconcile_args = serde_json::json!({ "analysis_path": "knowerage/analysis/app.md" });
        server
            .dispatch_tool("knowerage.reconcile_record", reconcile_args)
            .unwrap();

        let status_args = serde_json::json!({ "source_path": "src/App.java" });
        let result = server
            .dispatch_tool("knowerage.get_file_status", status_args)
            .unwrap();

        assert_eq!(result["total_lines"], 10);
        assert_eq!(result["analyzed_ranges"], serde_json::json!([[1, 5]]));
        assert_eq!(result["missing_ranges"], serde_json::json!([[6, 10]]));
        assert_eq!(result["coverage_percent"], 50.0);
    }

    // Test 9: coverage_overview with empty registry
    #[test]
    fn test_coverage_overview_empty() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let result = server
            .dispatch_tool("knowerage.coverage_overview", serde_json::json!({}))
            .unwrap();

        assert_eq!(result["summary"]["total_sources"], 0);
        assert_eq!(result["summary"]["tracked_files"], 0);
        assert_eq!(result["summary"]["total_records"], 0);
        assert_eq!(result["summary"]["avg_coverage_pct"], 0.0);
        assert_eq!(result["summary"]["stale_count"], 0);
        assert_eq!(result["summary"]["missing_src_count"], 0);
        assert_eq!(result["summary"]["project_files"], 0);
        assert_eq!(result["summary"]["project_lines"], 0);
        assert_eq!(result["summary"]["covered_lines"], 0);
        assert_eq!(result["summary"]["pct_files_tracked"], 0.0);
        assert_eq!(result["summary"]["pct_lines_covered_vs_project"], 0.0);
        assert!(!result["summary"]["extensions_applied"].as_array().unwrap().is_empty());
        assert!(result["sources"].as_array().unwrap().is_empty());
        assert!(result["stale_records"].as_array().unwrap().is_empty());
    }

    // Test 10: coverage_overview with a single fresh record
    #[test]
    fn test_coverage_overview_single_source() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let source = (1..=10).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        create_source_file(&tmp, "src/App.java", &source);

        let create = serde_json::json!({
            "analysis_path": "knowerage/analysis/app.md",
            "source_path": "src/App.java",
            "covered_lines": [[1, 5]],
            "content": "# Analysis"
        });
        server.dispatch_tool("knowerage.create_or_update_doc", create).unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                serde_json::json!({"analysis_path": "knowerage/analysis/app.md"}),
            )
            .unwrap();

        let result = server
            .dispatch_tool("knowerage.coverage_overview", serde_json::json!({}))
            .unwrap();

        assert_eq!(result["summary"]["total_sources"], 1);
        assert_eq!(result["summary"]["tracked_files"], 1);
        assert_eq!(result["summary"]["total_records"], 1);
        assert_eq!(result["summary"]["avg_coverage_pct"], 50.0);
        assert_eq!(result["summary"]["stale_count"], 0);
        assert_eq!(result["summary"]["project_files"], 1);
        assert_eq!(result["summary"]["project_lines"], 10);
        assert_eq!(result["summary"]["tracked_lines"], 10);
        assert_eq!(result["summary"]["covered_lines"], 5);
        assert_eq!(result["summary"]["pct_files_tracked"], 100.0);
        assert_eq!(result["summary"]["pct_lines_covered_vs_project"], 50.0);

        let sources = result["sources"].as_array().unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0]["total_lines"], 10);
        assert_eq!(sources[0]["coverage_percent"], 50.0);
        assert_eq!(sources[0]["analyzed_ranges"], serde_json::json!([[1, 5]]));
        assert_eq!(sources[0]["missing_ranges"], serde_json::json!([[6, 10]]));

        let attr = sources[0]["range_attribution"].as_array().unwrap();
        assert_eq!(attr.len(), 1);
        assert!(attr[0]["analysis_path"].as_str().unwrap().contains("app.md"));
    }

    // Test 11: coverage_overview uses fresh records only for coverage
    #[test]
    fn test_coverage_overview_fresh_only_policy() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let source = (1..=20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        create_source_file(&tmp, "src/Svc.java", &source);

        let create1 = serde_json::json!({
            "analysis_path": "knowerage/analysis/svc-a.md",
            "source_path": "src/Svc.java",
            "covered_lines": [[1, 10]],
            "content": "# A"
        });
        server.dispatch_tool("knowerage.create_or_update_doc", create1).unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                serde_json::json!({"analysis_path": "knowerage/analysis/svc-a.md"}),
            )
            .unwrap();

        let create2 = serde_json::json!({
            "analysis_path": "knowerage/analysis/svc-b.md",
            "source_path": "src/Svc.java",
            "covered_lines": [[11, 20]],
            "content": "# B"
        });
        server.dispatch_tool("knowerage.create_or_update_doc", create2).unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                serde_json::json!({"analysis_path": "knowerage/analysis/svc-b.md"}),
            )
            .unwrap();

        // Both fresh → 100% coverage
        let r1 = server
            .dispatch_tool("knowerage.coverage_overview", serde_json::json!({}))
            .unwrap();
        assert_eq!(r1["summary"]["avg_coverage_pct"], 100.0);

        // Mutate the source to make records stale, then reconcile
        let new_source = (1..=25).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        create_source_file(&tmp, "src/Svc.java", &new_source);
        server
            .dispatch_tool("knowerage.reconcile_all", serde_json::json!({}))
            .unwrap();

        let r2 = server
            .dispatch_tool("knowerage.coverage_overview", serde_json::json!({}))
            .unwrap();

        // Both records are now stale_src, so coverage from fresh = 0
        assert_eq!(r2["summary"]["stale_count"], 2);
        let sources = r2["sources"].as_array().unwrap();
        assert_eq!(sources[0]["coverage_percent"], 0.0);
        assert!(sources[0]["analyzed_ranges"].as_array().unwrap().is_empty());
    }

    // Test 12: coverage_overview with multiple source files
    #[test]
    fn test_coverage_overview_multiple_sources() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let src_a = (1..=10).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let src_b = (1..=20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        create_source_file(&tmp, "src/A.java", &src_a);
        create_source_file(&tmp, "src/B.java", &src_b);

        server
            .dispatch_tool(
                "knowerage.create_or_update_doc",
                serde_json::json!({
                    "analysis_path": "knowerage/analysis/a.md",
                    "source_path": "src/A.java",
                    "covered_lines": [[1, 10]],
                    "content": "# A"
                }),
            )
            .unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                serde_json::json!({"analysis_path": "knowerage/analysis/a.md"}),
            )
            .unwrap();

        server
            .dispatch_tool(
                "knowerage.create_or_update_doc",
                serde_json::json!({
                    "analysis_path": "knowerage/analysis/b.md",
                    "source_path": "src/B.java",
                    "covered_lines": [[1, 5]],
                    "content": "# B"
                }),
            )
            .unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                serde_json::json!({"analysis_path": "knowerage/analysis/b.md"}),
            )
            .unwrap();

        let result = server
            .dispatch_tool("knowerage.coverage_overview", serde_json::json!({}))
            .unwrap();

        assert_eq!(result["summary"]["total_sources"], 2);
        assert_eq!(result["summary"]["tracked_files"], 2);
        assert_eq!(result["summary"]["total_records"], 2);

        // A: 10/10 = 100%, B: 5/20 = 25% → avg = 15/30 = 50%
        assert_eq!(result["summary"]["avg_coverage_pct"], 50.0);
        assert_eq!(result["summary"]["project_files"], 2);
        assert_eq!(result["summary"]["project_lines"], 30);
        assert_eq!(result["summary"]["covered_lines"], 15);
        assert_eq!(result["summary"]["pct_files_tracked"], 100.0);
        assert_eq!(result["summary"]["pct_lines_covered_vs_project"], 50.0);
    }

    #[test]
    fn test_coverage_overview_project_includes_untracked_files() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let tracked = (1..=5).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let untracked = (1..=7).map(|i| format!("x {i}")).collect::<Vec<_>>().join("\n");
        create_source_file(&tmp, "src/Tracked.java", &tracked);
        create_source_file(&tmp, "src/Untracked.java", &untracked);

        server
            .dispatch_tool(
                "knowerage.create_or_update_doc",
                serde_json::json!({
                    "analysis_path": "knowerage/analysis/t.md",
                    "source_path": "src/Tracked.java",
                    "covered_lines": [[1, 2]],
                    "content": "# T"
                }),
            )
            .unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                serde_json::json!({"analysis_path": "knowerage/analysis/t.md"}),
            )
            .unwrap();

        let result = server
            .dispatch_tool("knowerage.coverage_overview", serde_json::json!({}))
            .unwrap();

        assert_eq!(result["summary"]["total_sources"], 1);
        assert_eq!(result["summary"]["project_files"], 2);
        assert_eq!(result["summary"]["project_lines"], 12);
        assert_eq!(result["summary"]["tracked_lines"], 5);
        assert_eq!(result["summary"]["covered_lines"], 2);
        assert_eq!(result["summary"]["pct_files_tracked"], 50.0);
    }

    #[test]
    fn test_coverage_overview_extensions_filter_java_only() {
        let tmp = TempDir::new().unwrap();
        let server = setup_server(&tmp);

        let java_src = "a\nb\nc\n";
        let xml_src = "x\ny\n";
        create_source_file(&tmp, "src/App.java", java_src);
        create_source_file(&tmp, "src/Data.xml", xml_src);

        server
            .dispatch_tool(
                "knowerage.create_or_update_doc",
                serde_json::json!({
                    "analysis_path": "knowerage/analysis/app.md",
                    "source_path": "src/App.java",
                    "covered_lines": [[1, 1]],
                    "content": "# A"
                }),
            )
            .unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                serde_json::json!({"analysis_path": "knowerage/analysis/app.md"}),
            )
            .unwrap();

        let r = server
            .dispatch_tool(
                "knowerage.coverage_overview",
                serde_json::json!({ "extensions": ["java"] }),
            )
            .unwrap();

        assert_eq!(r["summary"]["project_files"], 1);
        assert_eq!(r["summary"]["project_lines"], 3);
        assert_eq!(r["sources"].as_array().unwrap().len(), 1);
        assert_eq!(r["sources"][0]["source_path"], "src/App.java");

        let r_all = server
            .dispatch_tool("knowerage.coverage_overview", serde_json::json!({}))
            .unwrap();
        assert_eq!(r_all["summary"]["project_files"], 2);
        assert_eq!(r_all["summary"]["project_lines"], 5);
    }
}
