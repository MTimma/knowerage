use knowerage_mcp::registry::Registry;
use knowerage_mcp::types::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn write_analysis(root: &Path, name: &str, source: &str) {
    let dir = root.join("knowerage/analysis");
    fs::create_dir_all(&dir).unwrap();
    let content = format!(
        "---\nsource_file: \"{source}\"\ncovered_lines:\n  - [1, 50]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n# Analysis\n"
    );
    fs::write(dir.join(name), content).unwrap();
}

fn write_source(root: &Path, path: &str, content: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, content).unwrap();
}

#[test]
fn test_reconcile_all_after_source_changes() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_source(root, "src/a.java", "class A {}");
    write_source(root, "src/b.java", "class B {}");
    write_analysis(root, "a.md", "src/a.java");
    write_analysis(root, "b.md", "src/b.java");

    let registry = Registry::new(root.to_path_buf());
    let summary1 = registry.reconcile_all().unwrap();
    assert_eq!(summary1.total, 2);
    assert_eq!(summary1.fresh, 2);

    write_source(root, "src/b.java", "class B { int x; }");

    let summary2 = registry.reconcile_all().unwrap();
    assert_eq!(summary2.total, 2);
    assert_eq!(summary2.fresh, 1);
    assert_eq!(summary2.stale_src, 1);
}

#[test]
fn test_interrupted_write_no_corruption() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("knowerage")).unwrap();

    let registry = Registry::new(root.to_path_buf());
    let mut records1 = HashMap::new();
    let now = chrono::Utc::now();
    records1.insert(
        "knowerage/analysis/a.md".to_string(),
        RegistryRecord {
            analysis_path: std::path::PathBuf::from("knowerage/analysis/a.md"),
            source_path: std::path::PathBuf::from("src/a.java"),
            covered_ranges: vec![[1, 50]],
            analysis_hash: "sha256:aaa".to_string(),
            source_hash: "sha256:bbb".to_string(),
            record_created_at: now,
            record_updated_at: now,
            status: FreshnessStatus::Fresh,
        },
    );
    registry.save(&records1).unwrap();

    let reg_path = root.join("knowerage/registry.json");
    let content1 = fs::read_to_string(&reg_path).unwrap();
    let parsed1: HashMap<String, RegistryRecord> = serde_json::from_str(&content1).unwrap();
    assert_eq!(parsed1.len(), 1);

    let mut records2 = HashMap::new();
    records2.insert(
        "knowerage/analysis/b.md".to_string(),
        RegistryRecord {
            analysis_path: std::path::PathBuf::from("knowerage/analysis/b.md"),
            source_path: std::path::PathBuf::from("src/b.java"),
            covered_ranges: vec![[1, 100]],
            analysis_hash: "sha256:ccc".to_string(),
            source_hash: "sha256:ddd".to_string(),
            record_created_at: now,
            record_updated_at: now,
            status: FreshnessStatus::Fresh,
        },
    );
    registry.save(&records2).unwrap();

    let content2 = fs::read_to_string(&reg_path).unwrap();
    let parsed2: HashMap<String, RegistryRecord> = serde_json::from_str(&content2).unwrap();
    assert_eq!(parsed2.len(), 1);
    assert!(parsed2.contains_key("knowerage/analysis/b.md"));
}

#[test]
fn test_full_rebuild_from_analysis_files() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_source(root, "src/foo.java", "class Foo {}");
    write_source(root, "src/bar.java", "class Bar {}");
    write_analysis(root, "foo.md", "src/foo.java");
    write_analysis(root, "bar.md", "src/bar.java");

    let registry = Registry::new(root.to_path_buf());
    registry.reconcile_all().unwrap();

    let reg_path = root.join("knowerage/registry.json");
    fs::remove_file(&reg_path).unwrap();

    let summary = registry.reconcile_all().unwrap();

    assert_eq!(summary.total, 2);
    assert!(reg_path.exists());

    let records = registry.load().unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.contains_key("knowerage/analysis/foo.md"));
    assert!(records.contains_key("knowerage/analysis/bar.md"));
}

#[test]
fn test_mixed_status_summary() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    write_source(root, "src/fresh.java", "class Fresh {}");
    write_analysis(root, "fresh.md", "src/fresh.java");

    write_source(root, "src/stale_doc.java", "class StaleDoc {}");
    write_analysis(root, "stale_doc.md", "src/stale_doc.java");

    write_source(root, "src/stale_src.java", "class StaleSrc {}");
    write_analysis(root, "stale_src.md", "src/stale_src.java");

    write_source(root, "src/missing_src.java", "class MissingSrc {}");
    write_analysis(root, "missing_src.md", "src/missing_src.java");

    fs::write(
        root.join("knowerage/analysis/dangling.md"),
        "no valid frontmatter",
    )
    .unwrap();

    let registry = Registry::new(root.to_path_buf());
    registry.reconcile_all().unwrap();

    let stale_doc_path = root.join("knowerage/analysis/stale_doc.md");
    fs::write(
        &stale_doc_path,
        "---\nsource_file: \"src/stale_doc.java\"\ncovered_lines:\n  - [1, 50]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n# Modified\n",
    )
    .unwrap();

    write_source(root, "src/stale_src.java", "class StaleSrc { int x; }");
    fs::remove_file(root.join("src/missing_src.java")).unwrap();

    let summary = registry.reconcile_all().unwrap();

    assert_eq!(summary.total, 5);
    assert_eq!(summary.fresh, 1);
    assert_eq!(summary.stale_doc, 1);
    assert_eq!(summary.stale_src, 1);
    assert_eq!(summary.missing_src, 1);
    assert_eq!(summary.dangling_doc, 1);
}

#[test]
fn test_large_repo_incremental() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    for i in 0..105 {
        let src_path = format!("src/file_{i}.java");
        let analysis_name = format!("file_{i}.md");
        write_source(root, &src_path, &format!("class File{i} {{}}"));
        write_analysis(root, &analysis_name, &src_path);
    }

    let registry = Registry::new(root.to_path_buf());
    let summary = registry.reconcile_all().unwrap();

    assert!(summary.total >= 100);
    let records = registry.load().unwrap();
    assert!(records.len() >= 100);
}
