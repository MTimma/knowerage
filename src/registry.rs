use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use chrono::Utc;
use glob::glob;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::parser::parse_frontmatter;
use crate::security::RegistryLock;
use crate::types::{FreshnessStatus, KnowerageError, ParsedMetadata, RegistryRecord};

/// Debounce delay before running `reconcile_all` after filesystem activity settles.
const WATCHER_DEBOUNCE_MS: u64 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileSummary {
    pub total: usize,
    pub fresh: usize,
    pub stale_doc: usize,
    pub stale_src: usize,
    pub missing_src: usize,
    pub dangling_doc: usize,
}

pub struct Registry {
    workspace_root: PathBuf,
    registry_lock: Arc<RegistryLock>,
}

/// When unset or empty, auto full reconcile (file watcher) is **on** (backward compatible).
/// Set to `0`, `false`, or `no` (case-insensitive) to disable.
pub fn auto_full_reconcile_enabled() -> bool {
    match std::env::var("KNOWERAGE_AUTO_FULL_RECONCILE") {
        Ok(s) if s.trim().is_empty() => true,
        Ok(s) => {
            let lower = s.trim().to_lowercase();
            !matches!(lower.as_str(), "0" | "false" | "no")
        }
        Err(_) => true,
    }
}

fn path_is_registry_artifact(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("registry.json") | Some("registry.json.tmp")
    )
}

fn event_paths_all_registry_artifacts(event: &Event) -> bool {
    !event.paths.is_empty() && event.paths.iter().all(|p| path_is_registry_artifact(p))
}

fn watcher_reconcile_loop(rx: mpsc::Receiver<()>, workspace_root: PathBuf, lock: Arc<RegistryLock>) {
    while rx.recv().is_ok() {
        loop {
            match rx.recv_timeout(Duration::from_millis(WATCHER_DEBOUNCE_MS)) {
                Ok(()) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        let reg = Registry::with_lock(workspace_root.clone(), Arc::clone(&lock));
        if let Err(e) = reg.reconcile_all() {
            log::error!("Watcher reconcile failed: {e}");
        }
    }
}

impl Registry {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            registry_lock: Arc::new(RegistryLock::new()),
        }
    }

    pub fn with_lock(workspace_root: PathBuf, registry_lock: Arc<RegistryLock>) -> Self {
        Self {
            workspace_root,
            registry_lock,
        }
    }

    fn registry_path(&self) -> PathBuf {
        self.workspace_root.join("knowerage").join("registry.json")
    }

    fn load_unlocked(&self) -> Result<HashMap<String, RegistryRecord>, KnowerageError> {
        let path = self.registry_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(&path)
            .map_err(|e| KnowerageError::RegistryIo(format!("Failed to read registry: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| KnowerageError::RegistryIo(format!("Failed to parse registry: {e}")))
    }

    fn save_unlocked(&self, records: &HashMap<String, RegistryRecord>) -> Result<(), KnowerageError> {
        let path = self.registry_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| KnowerageError::RegistryIo(format!("Failed to create dir: {e}")))?;
        }
        let json = serde_json::to_string_pretty(records)
            .map_err(|e| KnowerageError::RegistryIo(format!("Failed to serialize: {e}")))?;

        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)
            .map_err(|e| KnowerageError::RegistryIo(format!("Failed to write temp file: {e}")))?;
        fs::rename(&tmp_path, &path)
            .map_err(|e| KnowerageError::RegistryIo(format!("Failed to rename temp file: {e}")))?;
        Ok(())
    }

    pub fn load(&self) -> Result<HashMap<String, RegistryRecord>, KnowerageError> {
        let _g = self.registry_lock.acquire()?;
        self.load_unlocked()
    }

    pub fn save(&self, records: &HashMap<String, RegistryRecord>) -> Result<(), KnowerageError> {
        let _g = self.registry_lock.acquire()?;
        self.save_unlocked(records)
    }

    pub fn hash_file(path: &Path) -> Result<String, KnowerageError> {
        let content = fs::read(path).map_err(|e| {
            KnowerageError::RegistryIo(format!("Failed to read {}: {e}", path.display()))
        })?;
        let hash = Sha256::digest(&content);
        Ok(format!("sha256:{hash:x}"))
    }

    fn compute_record(
        &self,
        records: &HashMap<String, RegistryRecord>,
        analysis_path: &Path,
        metadata: &ParsedMetadata,
    ) -> Result<RegistryRecord, KnowerageError> {
        let key = analysis_path.to_string_lossy().to_string();
        let abs_analysis = self.workspace_root.join(analysis_path);
        let abs_source = self.workspace_root.join(&metadata.source_file);

        let analysis_hash = Self::hash_file(&abs_analysis)?;

        let (source_hash, status) = if !abs_source.exists() {
            (String::new(), FreshnessStatus::MissingSrc)
        } else {
            let src_hash = Self::hash_file(&abs_source)?;
            if let Some(existing) = records.get(&key) {
                if existing.analysis_hash != analysis_hash {
                    (src_hash, FreshnessStatus::StaleDoc)
                } else if existing.source_hash != src_hash {
                    (src_hash, FreshnessStatus::StaleSrc)
                } else {
                    (src_hash, FreshnessStatus::Fresh)
                }
            } else {
                (src_hash, FreshnessStatus::Fresh)
            }
        };

        let now = Utc::now();
        let created_at = records.get(&key).map_or(now, |r| r.record_created_at);

        Ok(RegistryRecord {
            analysis_path: analysis_path.to_path_buf(),
            source_path: metadata.source_file.clone(),
            covered_ranges: metadata.covered_lines.clone(),
            analysis_hash,
            source_hash,
            record_created_at: created_at,
            record_updated_at: now,
            status,
        })
    }

    fn dangling_record(
        &self,
        records: &HashMap<String, RegistryRecord>,
        rel_path: &Path,
    ) -> RegistryRecord {
        let key = rel_path.to_string_lossy().to_string();
        let now = Utc::now();
        let abs_path = self.workspace_root.join(rel_path);
        let analysis_hash = Self::hash_file(&abs_path).unwrap_or_default();
        let created_at = records.get(&key).map_or(now, |r| r.record_created_at);

        RegistryRecord {
            analysis_path: rel_path.to_path_buf(),
            source_path: PathBuf::new(),
            covered_ranges: vec![],
            analysis_hash,
            source_hash: String::new(),
            record_created_at: created_at,
            record_updated_at: now,
            status: FreshnessStatus::DanglingDoc,
        }
    }

    pub fn reconcile_record(
        &self,
        analysis_path: &Path,
        metadata: &ParsedMetadata,
    ) -> Result<RegistryRecord, KnowerageError> {
        let _g = self.registry_lock.acquire()?;
        let mut records = self.load_unlocked()?;
        let key = analysis_path.to_string_lossy().to_string();
        let record = self.compute_record(&records, analysis_path, metadata)?;
        records.insert(key, record.clone());
        self.save_unlocked(&records)?;
        Ok(record)
    }

    pub fn reconcile_all(&self) -> Result<ReconcileSummary, KnowerageError> {
        let _g = self.registry_lock.acquire()?;

        let pattern = self
            .workspace_root
            .join("knowerage/analysis/**/*.md")
            .to_string_lossy()
            .to_string();

        let mut summary = ReconcileSummary {
            total: 0,
            fresh: 0,
            stale_doc: 0,
            stale_src: 0,
            missing_src: 0,
            dangling_doc: 0,
        };

        let paths: Vec<PathBuf> = glob(&pattern)
            .map_err(|e| KnowerageError::RegistryIo(format!("Glob pattern error: {e}")))?
            .filter_map(|entry| entry.ok())
            .collect();

        let mut records = self.load_unlocked()?;

        for abs_path in paths {
            summary.total += 1;

            let rel_path = abs_path
                .strip_prefix(&self.workspace_root)
                .map_err(|e| KnowerageError::RegistryIo(format!("Path prefix error: {e}")))?
                .to_path_buf();
            let key = rel_path.to_string_lossy().to_string();

            let content = match fs::read_to_string(&abs_path) {
                Ok(c) => c,
                Err(_) => {
                    let record = self.dangling_record(&records, &rel_path);
                    records.insert(key, record);
                    summary.dangling_doc += 1;
                    continue;
                }
            };

            match parse_frontmatter(&content) {
                Ok(metadata) => {
                    let record = self.compute_record(&records, &rel_path, &metadata)?;
                    match record.status {
                        FreshnessStatus::Fresh => summary.fresh += 1,
                        FreshnessStatus::StaleDoc => summary.stale_doc += 1,
                        FreshnessStatus::StaleSrc => summary.stale_src += 1,
                        FreshnessStatus::MissingSrc => summary.missing_src += 1,
                        FreshnessStatus::DanglingDoc => summary.dangling_doc += 1,
                    }
                    records.insert(key, record);
                }
                Err(_) => {
                    let record = self.dangling_record(&records, &rel_path);
                    records.insert(key, record);
                    summary.dangling_doc += 1;
                }
            }
        }

        self.save_unlocked(&records)?;
        Ok(summary)
    }

    pub fn start_watcher(&self) -> Result<RecommendedWatcher, KnowerageError> {
        let (tx, rx) = mpsc::channel::<()>();
        let workspace_root = self.workspace_root.clone();
        let lock = Arc::clone(&self.registry_lock);
        let tx_shared = Arc::new(std::sync::Mutex::new(tx));
        let tx_cb = Arc::clone(&tx_shared);
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if event_paths_all_registry_artifacts(&event) {
                        return;
                    }
                    if let Ok(guard) = tx_cb.lock() {
                        let _ = guard.send(());
                    }
                }
            },
            Config::default(),
        )
        .map_err(|e| KnowerageError::RegistryIo(format!("Failed to create watcher: {e}")))?;

        let watch_path = self.workspace_root.join("knowerage");
        if watch_path.exists() {
            watcher
                .watch(&watch_path, RecursiveMode::Recursive)
                .map_err(|e| {
                    KnowerageError::RegistryIo(format!("Failed to watch directory: {e}"))
                })?;
        }

        std::thread::spawn(move || watcher_reconcile_loop(rx, workspace_root, lock));

        Ok(watcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::thread;
    use tempfile::TempDir;

    fn setup_workspace(tmp: &TempDir) {
        fs::create_dir_all(tmp.path().join("knowerage/analysis")).unwrap();
    }

    fn write_analysis_file(tmp: &TempDir, name: &str, source: &str) -> PathBuf {
        let dir = tmp.path().join("knowerage/analysis");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let content = format!(
            "---\nsource_file: \"{source}\"\ncovered_lines:\n  - [1, 50]\nanalysis_date: \"2024-01-15T10:00:00Z\"\n---\n# Analysis\n"
        );
        fs::write(&path, &content).unwrap();
        path
    }

    fn make_metadata(source: &str) -> ParsedMetadata {
        ParsedMetadata {
            source_file: PathBuf::from(source),
            covered_lines: vec![[1, 50]],
            analysis_date: Utc::now(),
        }
    }

    #[test]
    fn test_hash_match_fresh() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let source_rel = "src/example.java";
        let source_abs = tmp.path().join(source_rel);
        fs::create_dir_all(source_abs.parent().unwrap()).unwrap();
        fs::write(&source_abs, "public class Example {}").unwrap();

        let analysis_abs = write_analysis_file(&tmp, "example.md", source_rel);
        let analysis_rel = analysis_abs.strip_prefix(tmp.path()).unwrap();

        let registry = Registry::new(tmp.path().to_path_buf());
        let metadata = make_metadata(source_rel);

        let record = registry.reconcile_record(analysis_rel, &metadata).unwrap();
        assert_eq!(record.status, FreshnessStatus::Fresh);

        let record = registry.reconcile_record(analysis_rel, &metadata).unwrap();
        assert_eq!(record.status, FreshnessStatus::Fresh);
    }

    #[test]
    fn test_analysis_hash_mismatch_stale_doc() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let source_rel = "src/example.java";
        let source_abs = tmp.path().join(source_rel);
        fs::create_dir_all(source_abs.parent().unwrap()).unwrap();
        fs::write(&source_abs, "public class Example {}").unwrap();

        let analysis_abs = write_analysis_file(&tmp, "example.md", source_rel);
        let analysis_rel = analysis_abs.strip_prefix(tmp.path()).unwrap().to_path_buf();

        let registry = Registry::new(tmp.path().to_path_buf());
        let metadata = make_metadata(source_rel);

        registry.reconcile_record(&analysis_rel, &metadata).unwrap();

        // Modify the analysis file body (hash changes, frontmatter stays valid)
        fs::write(
            &analysis_abs,
            "---\nsource_file: \"src/example.java\"\ncovered_lines:\n  - [1, 50]\nanalysis_date: \"2024-01-15T10:00:00Z\"\n---\n# MODIFIED BODY\n",
        )
        .unwrap();

        let record = registry.reconcile_record(&analysis_rel, &metadata).unwrap();
        assert_eq!(record.status, FreshnessStatus::StaleDoc);
    }

    #[test]
    fn test_source_hash_mismatch_stale_src() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let source_rel = "src/example.java";
        let source_abs = tmp.path().join(source_rel);
        fs::create_dir_all(source_abs.parent().unwrap()).unwrap();
        fs::write(&source_abs, "public class Example {}").unwrap();

        let analysis_abs = write_analysis_file(&tmp, "example.md", source_rel);
        let analysis_rel = analysis_abs.strip_prefix(tmp.path()).unwrap().to_path_buf();

        let registry = Registry::new(tmp.path().to_path_buf());
        let metadata = make_metadata(source_rel);

        registry.reconcile_record(&analysis_rel, &metadata).unwrap();

        // Modify the source file only
        fs::write(&source_abs, "public class Example { int x; }").unwrap();

        let record = registry.reconcile_record(&analysis_rel, &metadata).unwrap();
        assert_eq!(record.status, FreshnessStatus::StaleSrc);
    }

    #[test]
    fn test_source_deleted_missing_src() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let source_rel = "src/example.java";
        let source_abs = tmp.path().join(source_rel);
        fs::create_dir_all(source_abs.parent().unwrap()).unwrap();
        fs::write(&source_abs, "public class Example {}").unwrap();

        let analysis_abs = write_analysis_file(&tmp, "example.md", source_rel);
        let analysis_rel = analysis_abs.strip_prefix(tmp.path()).unwrap().to_path_buf();

        let registry = Registry::new(tmp.path().to_path_buf());
        let metadata = make_metadata(source_rel);

        registry.reconcile_record(&analysis_rel, &metadata).unwrap();

        fs::remove_file(&source_abs).unwrap();

        let record = registry.reconcile_record(&analysis_rel, &metadata).unwrap();
        assert_eq!(record.status, FreshnessStatus::MissingSrc);
    }

    #[test]
    fn test_malformed_doc_dangling() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let bad_path = tmp.path().join("knowerage/analysis/bad.md");
        fs::write(&bad_path, "This file has no valid frontmatter at all").unwrap();

        let registry = Registry::new(tmp.path().to_path_buf());
        let summary = registry.reconcile_all().unwrap();

        assert_eq!(summary.total, 1);
        assert_eq!(summary.dangling_doc, 1);

        let records = registry.load().unwrap();
        let record = records.get("knowerage/analysis/bad.md").unwrap();
        assert_eq!(record.status, FreshnessStatus::DanglingDoc);
    }

    #[test]
    fn test_atomic_write_valid_json() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let registry = Registry::new(tmp.path().to_path_buf());
        let mut records = HashMap::new();
        let now = Utc::now();
        records.insert(
            "knowerage/analysis/test.md".to_string(),
            RegistryRecord {
                analysis_path: PathBuf::from("knowerage/analysis/test.md"),
                source_path: PathBuf::from("src/test.java"),
                covered_ranges: vec![[1, 100]],
                analysis_hash: "sha256:abc123".to_string(),
                source_hash: "sha256:def456".to_string(),
                record_created_at: now,
                record_updated_at: now,
                status: FreshnessStatus::Fresh,
            },
        );

        registry.save(&records).unwrap();

        let content = fs::read_to_string(registry.registry_path()).unwrap();
        let parsed: HashMap<String, RegistryRecord> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed.contains_key("knowerage/analysis/test.md"));

        let tmp_file = registry.registry_path().with_extension("json.tmp");
        assert!(
            !tmp_file.exists(),
            "Temp file should not remain after atomic write"
        );
    }

    #[test]
    fn test_reconcile_all_summary() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();

        // A: stays fresh
        fs::write(src_dir.join("a.java"), "class A {}").unwrap();
        write_analysis_file(&tmp, "a.md", "src/a.java");

        // B: analysis will be modified → StaleDoc
        fs::write(src_dir.join("b.java"), "class B {}").unwrap();
        write_analysis_file(&tmp, "b.md", "src/b.java");

        // C: source will be modified → StaleSrc
        fs::write(src_dir.join("c.java"), "class C {}").unwrap();
        write_analysis_file(&tmp, "c.md", "src/c.java");

        // D: source will be deleted → MissingSrc
        fs::write(src_dir.join("d.java"), "class D {}").unwrap();
        write_analysis_file(&tmp, "d.md", "src/d.java");

        // E: invalid frontmatter → DanglingDoc
        let bad_path = tmp.path().join("knowerage/analysis/e.md");
        fs::write(&bad_path, "no frontmatter here").unwrap();

        let registry = Registry::new(tmp.path().to_path_buf());

        let initial = registry.reconcile_all().unwrap();
        assert_eq!(initial.total, 5);
        assert_eq!(initial.fresh, 4);
        assert_eq!(initial.dangling_doc, 1);

        // Modify analysis B body (changes file hash)
        let b_path = tmp.path().join("knowerage/analysis/b.md");
        let b_content = fs::read_to_string(&b_path).unwrap();
        fs::write(&b_path, format!("{b_content}\n# Extra content appended")).unwrap();

        // Modify source C
        fs::write(src_dir.join("c.java"), "class C { int modified; }").unwrap();

        // Delete source D
        fs::remove_file(src_dir.join("d.java")).unwrap();

        let summary = registry.reconcile_all().unwrap();
        assert_eq!(summary.total, 5);
        assert_eq!(summary.fresh, 1); // A
        assert_eq!(summary.stale_doc, 1); // B
        assert_eq!(summary.stale_src, 1); // C
        assert_eq!(summary.missing_src, 1); // D
        assert_eq!(summary.dangling_doc, 1); // E
    }

    #[test]
    fn test_large_file_hash() {
        let tmp = TempDir::new().unwrap();
        let large_file = tmp.path().join("large.java");
        let line = "// This is a line of Java source code with some realistic content padding\n";
        let content: String = line.repeat(2000);
        assert!(content.len() > 80_000, "File must be at least ~80KB");

        fs::write(&large_file, &content).unwrap();

        let start = std::time::Instant::now();
        let hash = Registry::hash_file(&large_file).unwrap();
        let elapsed = start.elapsed();

        assert!(hash.starts_with("sha256:"));
        assert!(hash.len() > 10);
        assert!(
            elapsed.as_secs() < 5,
            "Hashing ~80KB took too long: {elapsed:?}"
        );
    }

    #[test]
    fn test_concurrent_reconcile_record_no_lost_keys() {
        let tmp = TempDir::new().unwrap();
        setup_workspace(&tmp);

        let src_dir = tmp.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("one.java"), "class One {}").unwrap();
        fs::write(src_dir.join("two.java"), "class Two {}").unwrap();

        write_analysis_file(&tmp, "one.md", "src/one.java");
        write_analysis_file(&tmp, "two.md", "src/two.java");

        let lock = Arc::new(RegistryLock::new());
        let root = tmp.path().to_path_buf();

        let t1 = {
            let root = root.clone();
            let lock = Arc::clone(&lock);
            thread::spawn(move || {
                let reg = Registry::with_lock(root, lock);
                let m = ParsedMetadata {
                    source_file: PathBuf::from("src/one.java"),
                    covered_lines: vec![[1, 50]],
                    analysis_date: Utc::now(),
                };
                for _ in 0..50 {
                    reg.reconcile_record(Path::new("knowerage/analysis/one.md"), &m)
                        .unwrap();
                }
            })
        };

        let t2 = {
            let root = root.clone();
            let lock = Arc::clone(&lock);
            thread::spawn(move || {
                let reg = Registry::with_lock(root, lock);
                let m = ParsedMetadata {
                    source_file: PathBuf::from("src/two.java"),
                    covered_lines: vec![[1, 50]],
                    analysis_date: Utc::now(),
                };
                for _ in 0..50 {
                    reg.reconcile_record(Path::new("knowerage/analysis/two.md"), &m)
                        .unwrap();
                }
            })
        };

        t1.join().unwrap();
        t2.join().unwrap();

        let reg = Registry::with_lock(root, lock);
        let records = reg.load().unwrap();
        assert!(records.contains_key("knowerage/analysis/one.md"));
        assert!(records.contains_key("knowerage/analysis/two.md"));
    }
}
