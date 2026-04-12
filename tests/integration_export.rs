use knowerage_mcp::export::*;
use knowerage_mcp::registry::Registry;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_workspace(tmp: &TempDir) {
    fs::create_dir_all(tmp.path().join("knowerage/analysis")).unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
}

fn write_analysis(root: &std::path::Path, name: &str, source: &str) -> PathBuf {
    let path = root.join("knowerage/analysis").join(name);
    let content = format!(
        "---\nsource_file: \"{source}\"\ncovered_lines:\n  - [1, 50]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n# Analysis of {source}\nDetailed analysis content here.\n"
    );
    fs::write(&path, content).unwrap();
    PathBuf::from(format!("knowerage/analysis/{name}"))
}

fn write_source(root: &std::path::Path, path: &str) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, "public class Example {}").unwrap();
}

#[test]
fn test_export_50_selected() {
    let tmp = TempDir::new().unwrap();
    setup_workspace(&tmp);

    for i in 0..60 {
        let name = format!("analysis_{i:03}.md");
        let source = format!("src/File{i:03}.java");
        write_source(tmp.path(), &source);
        write_analysis(tmp.path(), &name, &source);
    }

    let registry = Registry::new(tmp.path().to_path_buf());
    registry.reconcile_all().unwrap();

    let selection = ExportSelection {
        paths: vec![],
        limit: Some(50),
    };
    let selected = select_files(&selection, &registry, tmp.path()).unwrap();
    assert!(selected.len() <= 50);

    let bundle = generate_bundle(&selected, tmp.path()).unwrap();
    assert!(!bundle.primary_toc().is_empty());
    assert!(!bundle.primary_combined().is_empty());
    assert!(bundle.manifest.files.len() <= 50);
}

#[test]
fn test_bundle_structurally_valid() {
    let tmp = TempDir::new().unwrap();
    setup_workspace(&tmp);

    write_source(tmp.path(), "src/A.java");
    write_source(tmp.path(), "src/B.java");
    let p1 = write_analysis(tmp.path(), "a.md", "src/A.java");
    let p2 = write_analysis(tmp.path(), "b.md", "src/B.java");

    let bundle = generate_bundle(&[p1, p2], tmp.path()).unwrap();

    assert!(
        bundle.primary_toc().contains("Analysis"),
        "TOC should have Analysis column"
    );
    assert!(
        bundle.primary_toc().contains("Source"),
        "TOC should have Source column"
    );

    let combined_all: String = bundle
        .parts
        .iter()
        .map(|p| p.combined.as_str())
        .collect::<Vec<_>>()
        .join("");
    assert!(combined_all.contains("src/A.java"));
    assert!(combined_all.contains("src/B.java"));
}

#[test]
fn test_mixed_valid_invalid_input() {
    let tmp = TempDir::new().unwrap();
    setup_workspace(&tmp);

    write_source(tmp.path(), "src/A.java");
    let valid = write_analysis(tmp.path(), "a.md", "src/A.java");
    let invalid = PathBuf::from("knowerage/analysis/nonexistent.md");

    let bundle = generate_bundle(&[valid, invalid], tmp.path()).unwrap();

    assert_eq!(bundle.manifest.files.len(), 1, "Should have 1 valid file");
    assert_eq!(bundle.manifest.errors.len(), 1, "Should have 1 error");
}

#[test]
fn test_manifest_traceability() {
    let tmp = TempDir::new().unwrap();
    setup_workspace(&tmp);

    write_source(tmp.path(), "src/A.java");
    write_source(tmp.path(), "src/B.java");
    write_source(tmp.path(), "src/C.java");
    let paths: Vec<PathBuf> = ["a.md", "b.md", "c.md"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let src = format!("src/{}.java", (b'A' + i as u8) as char);
            write_analysis(tmp.path(), name, &src)
        })
        .collect();

    let bundle = generate_bundle(&paths, tmp.path()).unwrap();

    assert_eq!(bundle.manifest.files.len(), 3);
    for entry in &bundle.manifest.files {
        assert!(
            entry.content_hash.starts_with("sha256:"),
            "Hash should start with sha256:"
        );
        assert!(!entry.analysis_path.to_string_lossy().is_empty());
        assert!(!entry.source_path.to_string_lossy().is_empty());
    }

    let now = chrono::Utc::now();
    let diff = now
        .signed_duration_since(bundle.manifest.created_at)
        .num_seconds()
        .unsigned_abs();
    assert!(diff < 60, "Manifest timestamp should be recent");
}

#[test]
fn test_deterministic_output() {
    let tmp = TempDir::new().unwrap();
    setup_workspace(&tmp);

    write_source(tmp.path(), "src/A.java");
    let path = write_analysis(tmp.path(), "a.md", "src/A.java");

    let b1 = generate_bundle(std::slice::from_ref(&path), tmp.path()).unwrap();
    let b2 = generate_bundle(std::slice::from_ref(&path), tmp.path()).unwrap();

    assert_eq!(b1.primary_toc(), b2.primary_toc());
    assert_eq!(b1.primary_combined(), b2.primary_combined());
    assert_eq!(b1.manifest.files.len(), b2.manifest.files.len());
    if let (Some(f1), Some(f2)) = (b1.manifest.files.first(), b2.manifest.files.first()) {
        assert_eq!(f1.content_hash, f2.content_hash);
    }
}
