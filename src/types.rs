use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedMetadata {
    pub source_file: PathBuf,
    pub covered_lines: Vec<[u64; 2]>,
    pub analysis_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryRecord {
    pub analysis_path: PathBuf,
    pub source_path: PathBuf,
    pub covered_ranges: Vec<[u64; 2]>,
    pub analysis_hash: String,
    pub source_hash: String,
    pub record_created_at: DateTime<Utc>,
    pub record_updated_at: DateTime<Utc>,
    pub status: FreshnessStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessStatus {
    Fresh,
    StaleDoc,
    StaleSrc,
    MissingSrc,
    DanglingDoc,
}

impl fmt::Display for FreshnessStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fresh => write!(f, "fresh"),
            Self::StaleDoc => write!(f, "stale_doc"),
            Self::StaleSrc => write!(f, "stale_src"),
            Self::MissingSrc => write!(f, "missing_src"),
            Self::DanglingDoc => write!(f, "dangling_doc"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeAttribution {
    pub range: [u64; 2],
    pub analysis_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    pub source_path: PathBuf,
    pub total_lines: u64,
    pub analyzed_ranges: Vec<[u64; 2]>,
    pub missing_ranges: Vec<[u64; 2]>,
    pub coverage_percent: f64,
    pub range_attribution: Vec<RangeAttribution>,
}

#[derive(Debug, Error)]
pub enum KnowerageError {
    #[error("{0}")]
    DocParse(String),

    #[error("{0}")]
    RangeInvalid(String),

    #[error("{0}")]
    SrcMissing(String),

    #[error("{0}")]
    PathTraversal(String),

    #[error("{0}")]
    RegistryIo(String),
}

impl KnowerageError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DocParse(_) => "E_DOC_PARSE",
            Self::RangeInvalid(_) => "E_RANGE_INVALID",
            Self::SrcMissing(_) => "E_SRC_MISSING",
            Self::PathTraversal(_) => "E_PATH_TRAVERSAL",
            Self::RegistryIo(_) => "E_REGISTRY_IO",
        }
    }
}
