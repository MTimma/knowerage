use knowerage_mcp::mcp::McpServer;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn setup(tmp: &TempDir) -> McpServer {
    let root = fs::canonicalize(tmp.path()).unwrap();
    fs::create_dir_all(root.join("knowerage/analysis")).unwrap();
    McpServer::new(root)
}

fn create_source(tmp: &TempDir, rel: &str, content: &str) {
    let root = fs::canonicalize(tmp.path()).unwrap();
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

#[test]
fn test_full_flow_create_reconcile_status() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    let source = (1..=20)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    create_source(&tmp, "src/Main.java", &source);

    let result = server
        .dispatch_tool(
            "knowerage.create_or_update_doc",
            json!({
                "analysis_path": "knowerage/analysis/main.md",
                "source_path": "src/Main.java",
                "covered_lines": [[1, 10]],
                "content": "# Main analysis"
            }),
        )
        .unwrap();
    assert_eq!(result["ok"], true);

    let rec = server
        .dispatch_tool(
            "knowerage.reconcile_record",
            json!({
                "analysis_path": "knowerage/analysis/main.md"
            }),
        )
        .unwrap();
    assert_eq!(rec["status"], "fresh");

    let status = server
        .dispatch_tool(
            "knowerage.get_file_status",
            json!({
                "source_path": "src/Main.java"
            }),
        )
        .unwrap();
    assert_eq!(status["total_lines"], 20);
    assert_eq!(status["analyzed_ranges"], json!([[1, 10]]));
    assert_eq!(status["missing_ranges"], json!([[11, 20]]));
    assert_eq!(status["coverage_percent"], 50.0);

    let listed = server
        .dispatch_tool("knowerage.list_registry", json!({}))
        .unwrap();
    assert_eq!(listed["record_count"], 1);
    assert_eq!(listed["registry_file"], "knowerage/registry.json");
    assert!(listed["schema_note"].as_str().is_some());
    let records = listed["records"].as_object().unwrap();
    assert_eq!(records.len(), 1);
    let row = records.get("knowerage/analysis/main.md").unwrap();
    assert_eq!(row["source_path"], "src/Main.java");
    assert_eq!(row["covered_ranges"], json!([[1, 10]]));
    assert_eq!(row["status"], "fresh");
}

#[test]
fn test_malformed_analysis_parse_error() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    let root = fs::canonicalize(tmp.path()).unwrap();
    fs::write(
        root.join("knowerage/analysis/bad.md"),
        "Not valid frontmatter\n",
    )
    .unwrap();

    let err = server
        .dispatch_tool(
            "knowerage.parse_doc_metadata",
            json!({
                "analysis_path": "knowerage/analysis/bad.md"
            }),
        )
        .unwrap_err();
    assert_eq!(err.code(), "E_DOC_PARSE");
}

#[test]
fn test_path_traversal_in_tool_call() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    let err = server
        .dispatch_tool(
            "knowerage.create_or_update_doc",
            json!({
                "analysis_path": "../../x",
                "source_path": "src/test.java",
                "covered_lines": [[1, 1]],
                "content": "# test"
            }),
        )
        .unwrap_err();
    assert_eq!(err.code(), "E_PATH_TRAVERSAL");
}

#[test]
fn test_list_stale_returns_filtered() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    create_source(&tmp, "src/A.java", "class A {}");
    server
        .dispatch_tool(
            "knowerage.create_or_update_doc",
            json!({
                "analysis_path": "knowerage/analysis/a.md",
                "source_path": "src/A.java",
                "covered_lines": [[1, 1]],
                "content": "# A"
            }),
        )
        .unwrap();
    server
        .dispatch_tool(
            "knowerage.reconcile_record",
            json!({ "analysis_path": "knowerage/analysis/a.md" }),
        )
        .unwrap();

    create_source(&tmp, "src/B.java", "class B {}");
    server
        .dispatch_tool(
            "knowerage.create_or_update_doc",
            json!({
                "analysis_path": "knowerage/analysis/b.md",
                "source_path": "src/B.java",
                "covered_lines": [[1, 1]],
                "content": "# B"
            }),
        )
        .unwrap();
    server
        .dispatch_tool(
            "knowerage.reconcile_record",
            json!({ "analysis_path": "knowerage/analysis/b.md" }),
        )
        .unwrap();
    create_source(&tmp, "src/B.java", "class B { modified }");
    server
        .dispatch_tool(
            "knowerage.reconcile_record",
            json!({ "analysis_path": "knowerage/analysis/b.md" }),
        )
        .unwrap();

    let stale = server
        .dispatch_tool(
            "knowerage.list_stale",
            json!({
                "statuses": ["stale_src"]
            }),
        )
        .unwrap();
    let arr = stale.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["status"], "stale_src");
}

#[test]
fn test_get_tree_with_group_by() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    create_source(&tmp, "src/auth/Login.java", "class Login {}");
    create_source(&tmp, "src/auth/Logout.java", "class Logout {}");
    create_source(&tmp, "src/util/Helper.java", "class Helper {}");

    for (analysis, source) in &[
        ("knowerage/analysis/login.md", "src/auth/Login.java"),
        ("knowerage/analysis/logout.md", "src/auth/Logout.java"),
        ("knowerage/analysis/helper.md", "src/util/Helper.java"),
    ] {
        server
            .dispatch_tool(
                "knowerage.create_or_update_doc",
                json!({
                    "analysis_path": analysis,
                    "source_path": source,
                    "covered_lines": [[1, 1]],
                    "content": "# Analysis"
                }),
            )
            .unwrap();
        server
            .dispatch_tool(
                "knowerage.reconcile_record",
                json!({ "analysis_path": analysis }),
            )
            .unwrap();
    }

    let tree = server
        .dispatch_tool(
            "knowerage.get_tree",
            json!({
                "root": "src/",
                "group_by": "directory"
            }),
        )
        .unwrap();

    let arr = tree.as_array().unwrap();
    assert!(arr.len() >= 2, "Should have at least 2 directory groups");
}

#[test]
fn test_export_report_produces_valid_file() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    create_source(&tmp, "src/App.java", "class App {}");
    server
        .dispatch_tool(
            "knowerage.create_or_update_doc",
            json!({
                "analysis_path": "knowerage/analysis/app.md",
                "source_path": "src/App.java",
                "covered_lines": [[1, 1]],
                "content": "# App"
            }),
        )
        .unwrap();
    server
        .dispatch_tool(
            "knowerage.reconcile_record",
            json!({ "analysis_path": "knowerage/analysis/app.md" }),
        )
        .unwrap();

    let result = server
        .dispatch_tool(
            "registry.export_report",
            json!({
                "format": "json",
                "output_path": "knowerage/report.json"
            }),
        )
        .unwrap();
    assert_eq!(result["ok"], true);

    let root = fs::canonicalize(tmp.path()).unwrap();
    let report = fs::read_to_string(root.join("knowerage/report.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert!(parsed.is_object());
}

#[test]
fn test_generate_bundle_integration() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    create_source(&tmp, "src/Note.java", "class Note {}");
    server
        .dispatch_tool(
            "knowerage.create_or_update_doc",
            json!({
                "analysis_path": "knowerage/analysis/note.md",
                "source_path": "src/Note.java",
                "covered_lines": [[1, 1]],
                "content": "# Note analysis"
            }),
        )
        .unwrap();

    let result = server
        .dispatch_tool(
            "knowerage.generate_bundle",
            json!({
                "analysis_paths": ["knowerage/analysis/note.md"],
                "output_dir": "knowerage/bundle-out"
            }),
        )
        .unwrap();
    assert_eq!(result["ok"], true);
    assert!(result["files_written"].as_array().unwrap().len() >= 3);

    let root = fs::canonicalize(tmp.path()).unwrap();
    assert!(root.join("knowerage/bundle-out/combined.md").is_file());
    assert!(root.join("knowerage/bundle-out/manifest.json").is_file());
}

#[test]
fn test_coverage_overview_extensions_integration() {
    let tmp = TempDir::new().unwrap();
    let server = setup(&tmp);

    create_source(&tmp, "src/App.java", "a\nb\n");
    create_source(&tmp, "src/Data.xml", "x\ny\nz\n");

    server
        .dispatch_tool(
            "knowerage.create_or_update_doc",
            json!({
                "analysis_path": "knowerage/analysis/app.md",
                "source_path": "src/App.java",
                "covered_lines": [[1, 1]],
                "content": "# App"
            }),
        )
        .unwrap();
    server
        .dispatch_tool(
            "knowerage.reconcile_record",
            json!({ "analysis_path": "knowerage/analysis/app.md" }),
        )
        .unwrap();

    let java_only = server
        .dispatch_tool(
            "knowerage.coverage_overview",
            json!({ "extensions": ["java"] }),
        )
        .unwrap();
    assert_eq!(java_only["summary"]["project_files"], 1);
    assert_eq!(java_only["summary"]["project_lines"], 2);
    assert_eq!(java_only["sources"].as_array().unwrap().len(), 1);

    let defaults = server
        .dispatch_tool("knowerage.coverage_overview", json!({}))
        .unwrap();
    assert_eq!(defaults["summary"]["project_files"], 2);
    assert_eq!(defaults["summary"]["project_lines"], 5);
}
