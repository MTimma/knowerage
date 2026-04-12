use std::path::PathBuf;

use chrono::DateTime;
use serde_yaml::Value;

use crate::types::{KnowerageError, ParsedMetadata};

fn extract_yaml(content: &str) -> Result<&str, KnowerageError> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(KnowerageError::DocParse(
            "Missing opening frontmatter delimiter".into(),
        ));
    }
    let rest = &trimmed[3..];
    let end = rest
        .find("\n---")
        .ok_or_else(|| KnowerageError::DocParse("Missing closing frontmatter delimiter".into()))?;
    Ok(&rest[..end])
}

fn validate_source_file(map: &serde_yaml::Mapping) -> Result<PathBuf, KnowerageError> {
    let val = map
        .get(Value::String("source_file".into()))
        .ok_or_else(|| KnowerageError::DocParse("Missing required field: source_file".into()))?;
    match val {
        Value::String(s) => Ok(PathBuf::from(s)),
        _ => Err(KnowerageError::DocParse(
            "source_file must be a string".into(),
        )),
    }
}

fn validate_analysis_date(
    map: &serde_yaml::Mapping,
) -> Result<DateTime<chrono::Utc>, KnowerageError> {
    let val = map
        .get(Value::String("analysis_date".into()))
        .ok_or_else(|| KnowerageError::DocParse("Missing required field: analysis_date".into()))?;
    match val {
        Value::String(s) => DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| KnowerageError::DocParse(format!("Invalid analysis_date: {e}"))),
        _ => Err(KnowerageError::DocParse(
            "analysis_date must be an ISO 8601 string".into(),
        )),
    }
}

fn parse_single_range(seq: &[Value]) -> Result<[u64; 2], KnowerageError> {
    if seq.len() != 2 {
        return Err(KnowerageError::RangeInvalid(
            "Each range must contain exactly 2 values".into(),
        ));
    }
    let start = value_to_u64(&seq[0])?;
    let end = value_to_u64(&seq[1])?;
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
}

fn value_to_u64(v: &Value) -> Result<u64, KnowerageError> {
    match v {
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(u)
            } else if n.as_f64().is_some() {
                Err(KnowerageError::RangeInvalid(
                    "Range values must be integers, not floats".into(),
                ))
            } else {
                Err(KnowerageError::RangeInvalid(
                    "Range values must be positive integers".into(),
                ))
            }
        }
        _ => Err(KnowerageError::RangeInvalid(
            "Range values must be integers".into(),
        )),
    }
}

fn validate_covered_lines(map: &serde_yaml::Mapping) -> Result<Vec<[u64; 2]>, KnowerageError> {
    let val = map
        .get(Value::String("covered_lines".into()))
        .ok_or_else(|| {
            KnowerageError::RangeInvalid("Missing required field: covered_lines".into())
        })?;

    let outer = match val {
        Value::Sequence(seq) => seq,
        Value::Null => {
            return Err(KnowerageError::RangeInvalid(
                "covered_lines must be a sequence, got null".into(),
            ));
        }
        _ => {
            return Err(KnowerageError::RangeInvalid(
                "covered_lines must be a sequence".into(),
            ));
        }
    };

    outer
        .iter()
        .map(|item| match item {
            Value::Sequence(inner) => parse_single_range(inner),
            _ => Err(KnowerageError::RangeInvalid(
                "Each covered_lines entry must be a [start, end] pair".into(),
            )),
        })
        .collect()
}

pub fn parse_frontmatter(content: &str) -> Result<ParsedMetadata, KnowerageError> {
    let yaml_str = extract_yaml(content)?;
    let doc: Value = serde_yaml::from_str(yaml_str)
        .map_err(|e| KnowerageError::DocParse(format!("Invalid YAML: {e}")))?;

    let map = doc
        .as_mapping()
        .ok_or_else(|| KnowerageError::DocParse("Frontmatter must be a YAML mapping".into()))?;

    let source_file = validate_source_file(map)?;
    let analysis_date = validate_analysis_date(map)?;
    let covered_lines = validate_covered_lines(map)?;

    Ok(ParsedMetadata {
        source_file,
        covered_lines,
        analysis_date,
    })
}

pub fn normalize_ranges(ranges: &[[u64; 2]]) -> Vec<[u64; 2]> {
    if ranges.is_empty() {
        return vec![];
    }

    let mut sorted: Vec<[u64; 2]> = ranges.to_vec();
    sorted.sort_by_key(|r| (r[0], r[1]));

    let mut merged: Vec<[u64; 2]> = vec![sorted[0]];
    for r in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        if r[0] <= last[1] + 1 {
            last[1] = last[1].max(r[1]);
        } else {
            merged.push(*r);
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_frontmatter() {
        let input = "---\nsource_file: src/App.java\ncovered_lines:\n  - [1, 10]\n  - [20, 30]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\nBody text";
        let result = parse_frontmatter(input).unwrap();
        assert_eq!(result.source_file, PathBuf::from("src/App.java"));
        assert_eq!(result.covered_lines, vec![[1, 10], [20, 30]]);
    }

    #[test]
    fn test_missing_source_file() {
        let input =
            "---\ncovered_lines:\n  - [1, 10]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n";
        let err = parse_frontmatter(input).unwrap_err();
        assert_eq!(err.code(), "E_DOC_PARSE");
    }

    #[test]
    fn test_covered_lines_string() {
        let input = "---\nsource_file: src/App.java\ncovered_lines: \"invalid\"\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n";
        let err = parse_frontmatter(input).unwrap_err();
        assert_eq!(err.code(), "E_RANGE_INVALID");
    }

    #[test]
    fn test_covered_lines_null() {
        let input = "---\nsource_file: src/App.java\ncovered_lines: null\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n";
        let err = parse_frontmatter(input).unwrap_err();
        assert_eq!(err.code(), "E_RANGE_INVALID");
    }

    #[test]
    fn test_covered_lines_object() {
        let input = "---\nsource_file: src/App.java\ncovered_lines:\n  a: 1\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n";
        let err = parse_frontmatter(input).unwrap_err();
        assert_eq!(err.code(), "E_RANGE_INVALID");
    }

    #[test]
    fn test_inverted_range() {
        let input = "---\nsource_file: src/App.java\ncovered_lines:\n  - [200, 120]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n";
        let err = parse_frontmatter(input).unwrap_err();
        assert_eq!(err.code(), "E_RANGE_INVALID");
    }

    #[test]
    fn test_overlapping_ranges_normalize() {
        let ranges = [[1, 10], [5, 15]];
        let result = normalize_ranges(&ranges);
        assert_eq!(result, vec![[1, 15]]);
    }

    #[test]
    fn test_adjacent_ranges_merge() {
        let ranges = [[1, 10], [11, 20]];
        let result = normalize_ranges(&ranges);
        assert_eq!(result, vec![[1, 20]]);
    }

    #[test]
    fn test_duplicate_ranges_collapse() {
        let ranges = [[1, 10], [1, 10]];
        let result = normalize_ranges(&ranges);
        assert_eq!(result, vec![[1, 10]]);
    }

    #[test]
    fn test_non_integer_in_range() {
        let input = "---\nsource_file: src/App.java\ncovered_lines:\n  - [1.5, 10]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n";
        let err = parse_frontmatter(input).unwrap_err();
        assert_eq!(err.code(), "E_RANGE_INVALID");
    }

    #[test]
    fn test_zero_start() {
        let input = "---\nsource_file: src/App.java\ncovered_lines:\n  - [0, 10]\nanalysis_date: \"2026-03-01T10:00:00Z\"\n---\n";
        let err = parse_frontmatter(input).unwrap_err();
        assert_eq!(err.code(), "E_RANGE_INVALID");
    }
}
