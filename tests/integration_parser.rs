use knowerage_mcp::parser::{normalize_ranges, parse_frontmatter};
use std::path::PathBuf;

#[test]
fn test_parse_real_analysis_file() {
    let content = r#"---
source_file: "src/main/java/com/example/AuthService.java"
covered_lines:
  - [120, 165]
  - [210, 248]
  - [300, 350]
analysis_date: "2026-03-15T14:30:00Z"
---
# Authentication Service Analysis

## Login Flow
The login method at lines 120-165 validates credentials...

## Session Management
Lines 210-248 handle session token generation...

## Password Reset
Lines 300-350 implement the password reset workflow...
"#;
    let result = parse_frontmatter(content).unwrap();
    assert_eq!(
        result.source_file,
        PathBuf::from("src/main/java/com/example/AuthService.java")
    );
    assert_eq!(result.covered_lines.len(), 3);
    assert_eq!(result.covered_lines[0], [120, 165]);
}

#[test]
fn test_parse_with_extra_frontmatter_keys() {
    let content = "---\nsource_file: \"src/App.java\"\ncovered_lines:\n  - [1, 10]\nanalysis_date: \"2026-03-01T10:00:00Z\"\nauthor: \"test\"\ntags:\n  - legacy\n  - auth\n---\n";
    let result = parse_frontmatter(content).unwrap();
    assert_eq!(result.source_file, PathBuf::from("src/App.java"));
}

#[test]
fn test_missing_closing_delimiter() {
    let content = "---\nsource_file: \"src/App.java\"\ncovered_lines:\n  - [1, 10]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n";
    let err = parse_frontmatter(content).unwrap_err();
    assert_eq!(err.code(), "E_DOC_PARSE");
}

#[test]
fn test_empty_file() {
    let err = parse_frontmatter("").unwrap_err();
    assert_eq!(err.code(), "E_DOC_PARSE");
}

#[test]
fn test_body_only_no_frontmatter() {
    let content = "# Just a regular markdown file\n\nNo frontmatter here.\n";
    let err = parse_frontmatter(content).unwrap_err();
    assert_eq!(err.code(), "E_DOC_PARSE");
}

#[test]
fn test_regression_overlap_merge() {
    assert_eq!(normalize_ranges(&[[5, 15], [1, 10]]), vec![[1, 15]]);
    assert_eq!(normalize_ranges(&[[1, 10], [11, 20]]), vec![[1, 20]]);
    assert_eq!(normalize_ranges(&[[1, 10], [1, 10]]), vec![[1, 10]]);
}

#[test]
fn test_regression_deterministic() {
    let content = "---\nsource_file: src/A.java\ncovered_lines:\n  - [5, 10]\n  - [1, 3]\nanalysis_date: \"2026-01-01T00:00:00Z\"\n---\n";
    let r1 = parse_frontmatter(content).unwrap();
    let r2 = parse_frontmatter(content).unwrap();
    assert_eq!(r1.covered_lines, r2.covered_lines);
    assert_eq!(r1.source_file, r2.source_file);
}
