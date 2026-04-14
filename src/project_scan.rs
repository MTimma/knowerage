//! Workspace-wide enumeration for coverage overview project totals.
//!
//! Skips common non-source directories by **name** (any depth): `.git`, `target`,
//! `node_modules`, `dist`, `build`, `knowerage`. Unreadable files are skipped.

use std::fs;
use std::path::Path;

use walkdir::WalkDir;

use crate::types::KnowerageError;

/// Default extensions when `extensions` is omitted or empty (lowercase, no dot).
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "java",
    "xml",
    "properties",
    "gradle",
    "kt",
    "groovy",
    "scala",
    "kts",
];

const EXCLUDED_DIR_NAMES: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    "knowerage",
];

pub fn default_extensions() -> Vec<String> {
    DEFAULT_EXTENSIONS
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

pub fn normalize_extension_list(input: &[String]) -> Vec<String> {
    input
        .iter()
        .map(|s| {
            let t = s.trim().to_lowercase();
            if let Some(stripped) = t.strip_prefix('.') {
                stripped.to_string()
            } else {
                t
            }
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Count files under `workspace_root` matching `extensions`, excluding known junk dirs.
/// Skips files that cannot be read as UTF-8 (or any read error).
pub fn project_files_and_lines(
    workspace_root: &Path,
    extensions: &[String],
) -> Result<(u64, u64), KnowerageError> {
    let mut files = 0u64;
    let mut lines = 0u64;

    let walker = WalkDir::new(workspace_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_str().unwrap_or("");
                !EXCLUDED_DIR_NAMES.contains(&name)
            } else {
                true
            }
        });

    for entry in walker {
        let entry = entry.map_err(|e| KnowerageError::RegistryIo(format!("walk: {e}")))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|x| x.to_str()) else {
            continue;
        };
        if !extensions.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
            continue;
        }
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        files += 1;
        lines += content.lines().count() as u64;
    }

    Ok((files, lines))
}
