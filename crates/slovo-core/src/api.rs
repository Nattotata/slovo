//! The JSON contract.
//!
//! These are the shapes the CLI emits with `--format json` and the ones the
//! HTTP API returns. Keeping them in one place means the two cannot drift.

use serde::{Deserialize, Serialize};

use crate::query::{per_million, FreqRow};
use crate::search::{SearchResult, SearchRow};

/// One row of a frequency list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreqEntry {
    pub value: String,
    pub freq: u64,
    pub per_million: f64,
}

/// Response for a frequency query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FreqResponse {
    pub dataset: String,
    pub by: String,
    pub tokens: u64,
    pub results: Vec<FreqEntry>,
}

impl FreqResponse {
    pub fn new(dataset: &str, by: &str, tokens: u64, rows: &[FreqRow]) -> Self {
        Self {
            dataset: dataset.to_string(),
            by: by.to_string(),
            tokens,
            results: rows
                .iter()
                .map(|row| FreqEntry {
                    value: row.value.clone(),
                    freq: row.count,
                    per_million: per_million(row.count, tokens),
                })
                .collect(),
        }
    }
}

/// One row of a pattern search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchEntry {
    pub text: String,
    pub freq: u64,
    pub per_million: f64,
}

/// Response for a pattern search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResponse {
    pub dataset: String,
    pub query: String,
    /// The canonical CQP the query expanded to.
    pub expanded: String,
    pub tokens: u64,
    pub matches: u64,
    pub distinct: usize,
    pub results: Vec<SearchEntry>,
}

impl SearchResponse {
    pub fn from_result(dataset: &str, result: &SearchResult) -> Self {
        Self {
            dataset: dataset.to_string(),
            query: result.pattern.clone(),
            expanded: result.expanded.clone(),
            tokens: result.tokens,
            matches: result.matches,
            distinct: result.distinct,
            results: result.rows.iter().map(entry_from_row).collect(),
        }
    }
}

fn entry_from_row(row: &SearchRow) -> SearchEntry {
    SearchEntry {
        text: row.text.clone(),
        freq: row.freq,
        per_million: row.per_million,
    }
}

/// One installed dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetSummary {
    pub name: String,
    pub tokens: u64,
}

/// Details about one dataset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetInfo {
    pub name: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub tokens: u64,
    pub documents: u64,
    pub sentences: u64,
    pub created_at: String,
}

/// Liveness probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Health {
    pub status: String,
    pub version: String,
}

/// The body of any error response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}
