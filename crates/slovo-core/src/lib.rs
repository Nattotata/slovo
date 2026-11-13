//! Slovo — a local corpus workstation.
//!
//! This crate is the engine: it owns the SQLite schema, the ingestion
//! pipeline and the query layer. Frontends (the `slovo` CLI and the Tauri
//! GUI) are thin wrappers over the API exposed here.

pub mod conllu;
pub mod error;
pub mod ingest;
pub mod query;
pub mod registry;
pub mod schema;
pub mod store;

pub use error::{Error, Result};
pub use ingest::{import_path, ImportOptions, ImportStats};
pub use query::{By, FreqRow};
pub use store::{CorpusInfo, Stats, Store};
