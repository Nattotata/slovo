//! Slovo — a local corpus workstation.
//!
//! This crate is the engine: it owns the SQLite schema, the ingestion
//! pipeline and the query layer. Frontends (the `slovo` CLI and the Tauri
//! GUI) are thin wrappers over the API exposed here.

pub mod api;
pub mod concordance;
pub mod conllu;
pub mod document;
pub mod error;
pub mod ingest;
pub mod query;
pub mod query_lang;
pub mod registry;
pub mod schema;
pub mod search;
pub mod store;

pub use concordance::{kwic, Hit, KwicLine, KwicOptions, KwicResult, KWIC_CONTEXT};
pub use document::{document, documents, Document, DocumentSentence, DocumentSummary};
pub use error::{Error, Result};
pub use ingest::{import_path, ImportOptions, ImportStats};
pub use query::{By, FreqRow, Rank};
pub use query_lang::QueryError;
pub use search::{search, SearchOptions, SearchResult, SearchRow};
pub use store::{CorpusInfo, Stats, Store};
