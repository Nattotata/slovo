//! The JSON contract.
//!
//! These are the shapes the CLI emits with `--format json` and the ones the
//! HTTP API returns. Keeping them in one place means the two cannot drift.

use serde::{Deserialize, Serialize};

use crate::concordance::{KwicLine, KwicResult};
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
    /// `frequency` (counts) or `weight` (a structural ranking, not a count).
    pub rank: String,
    pub results: Vec<FreqEntry>,
}

impl FreqResponse {
    pub fn new(dataset: &str, by: &str, tokens: u64, rows: &[FreqRow]) -> Self {
        Self::ranked(dataset, by, tokens, rows, crate::query::Rank::Frequency)
    }

    /// Build a response, labelling how the rows were ordered.
    ///
    /// Under `Rank::Weight` the per-row `freq` is a summed weight and
    /// `per_million` is not meaningful, so both are reported as-is and the
    /// `rank` field tells a reader which they are looking at.
    pub fn ranked(
        dataset: &str,
        by: &str,
        tokens: u64,
        rows: &[FreqRow],
        rank: crate::query::Rank,
    ) -> Self {
        let rank_label = match rank {
            crate::query::Rank::Frequency => "frequency",
            crate::query::Rank::Weight => "weight",
        };
        Self {
            dataset: dataset.to_string(),
            by: by.to_string(),
            tokens,
            rank: rank_label.to_string(),
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

/// One concordance example attached to a search row.
///
/// This is the "trace it back to the source" half of a cluster query: the
/// sentence a match came from, plus the document it belongs to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExampleEntry {
    pub sentence_id: i64,
    pub left: String,
    pub match_text: String,
    pub right: String,
}

impl From<&crate::concordance::Hit> for ExampleEntry {
    fn from(hit: &crate::concordance::Hit) -> Self {
        Self {
            sentence_id: hit.sentence_id,
            left: hit.left.clone(),
            match_text: hit.match_text.clone(),
            right: hit.right.clone(),
        }
    }
}

/// One row of a pattern search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchEntry {
    pub text: String,
    pub freq: u64,
    pub per_million: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ExampleEntry>,
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
        examples: row.examples.iter().map(ExampleEntry::from).collect(),
    }
}

/// One line of a concordance (KWIC) response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KwicEntry {
    pub document_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_external_id: Option<String>,
    pub sentence_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newdoc_id: Option<String>,
    pub left: String,
    pub match_text: String,
    pub right: String,
}

impl From<&KwicLine> for KwicEntry {
    fn from(line: &KwicLine) -> Self {
        Self {
            document_id: line.document_id,
            document_title: line.document_title.clone(),
            source_url: line.source_url.clone(),
            document_external_id: line.document_external_id.clone(),
            sentence_id: line.sentence_id,
            sent_id: line.sent_id.clone(),
            newdoc_id: line.newdoc_id.clone(),
            left: line.left.clone(),
            match_text: line.match_text.clone(),
            right: line.right.clone(),
        }
    }
}

/// Response for a concordance query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KwicResponse {
    pub dataset: String,
    pub query: String,
    /// The canonical CQP the query expanded to.
    pub expanded: String,
    pub tokens: u64,
    pub matches: u64,
    pub results: Vec<KwicEntry>,
}

impl KwicResponse {
    pub fn from_result(dataset: &str, result: &KwicResult) -> Self {
        Self {
            dataset: dataset.to_string(),
            query: result.pattern.clone(),
            expanded: result.expanded.clone(),
            tokens: result.tokens,
            matches: result.matches,
            results: result.lines.iter().map(KwicEntry::from).collect(),
        }
    }
}

/// One sentence of a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentSentenceEntry {
    pub sentence_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// One source document, with its sentences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentEntry {
    pub document_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub sentences: Vec<DocumentSentenceEntry>,
}

impl From<&crate::document::Document> for DocumentEntry {
    fn from(doc: &crate::document::Document) -> Self {
        Self {
            document_id: doc.id,
            title: doc.title.clone(),
            source_url: doc.source_url.clone(),
            license: doc.license.clone(),
            external_id: doc.external_id.clone(),
            sentences: doc
                .sentences
                .iter()
                .map(|s| DocumentSentenceEntry {
                    sentence_id: s.id,
                    sent_id: s.sent_id.clone(),
                    text: s.text.clone(),
                })
                .collect(),
        }
    }
}

/// One document in a listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentSummaryEntry {
    pub document_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub sentences: u64,
    pub tokens: u64,
}

impl From<&crate::document::DocumentSummary> for DocumentSummaryEntry {
    fn from(doc: &crate::document::DocumentSummary) -> Self {
        Self {
            document_id: doc.id,
            title: doc.title.clone(),
            source_url: doc.source_url.clone(),
            sentences: doc.sentences,
            tokens: doc.tokens,
        }
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
