//! The service layer.
//!
//! Every operation a client needs, in one place: the CLI, the HTTP API and
//! anything built later all call these.
//!
//! It knows about datasets, the registry and the downloader.
//! It knows nothing about argument parsing, JSON or HTTP.
//!
//! The reason it exists: the composition of "download a catalog entry, then
//! import it" used to live in the CLI binary, which meant only the CLI could
//! do it. Now the API and future clients can too.

use std::path::{Path, PathBuf};

use slovo_core::api::{DatasetInfo, DatasetSummary, FreqResponse, SearchResponse};
use slovo_core::query::By;
use slovo_core::registry::Registry;
use slovo_core::{ingest, query, search, Error, Result, Store};
use slovo_dataset::Manifest;
use slovo_fetch::FetchOptions;

/// Options for [`Service::add`].
#[derive(Debug, Clone, Default)]
pub struct AddOptions {
    /// Catalog id, e.g. `ud/UD_English-EWT`.
    pub source: String,
    pub catalog: Option<String>,
    pub to: Option<PathBuf>,
    pub force: bool,
    pub language: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
}

/// What [`Service::add`] produced.
#[derive(Debug, Clone)]
pub struct AddOutcome {
    pub manifest: Manifest,
    pub stats: ingest::ImportStats,
    pub directory: PathBuf,
}

/// Entry point for every client.
#[derive(Debug, Clone)]
pub struct Service {
    registry: Registry,
}

impl Default for Service {
    fn default() -> Self {
        Self::new()
    }
}

impl Service {
    /// A service over the registry from `SLOVO_DATA_DIR` (or the platform dir).
    pub fn new() -> Self {
        Self {
            registry: Registry::from_env(),
        }
    }

    /// A service rooted somewhere specific. Handy for tests.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            registry: Registry::at(root),
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    fn open(&self, name: &str) -> Result<Store> {
        Store::open_existing(self.registry.resolve(name)?)
    }

    /// Installed datasets, with token counts.
    pub fn list(&self) -> Result<Vec<DatasetSummary>> {
        let mut out = Vec::new();
        for name in self.registry.list()? {
            let tokens = Store::open_existing(self.registry.dataset_path(&name))
                .and_then(|store| store.token_count())
                .unwrap_or(0);
            out.push(DatasetSummary { name, tokens });
        }
        Ok(out)
    }

    /// Metadata and counts for one dataset.
    pub fn info(&self, name: &str) -> Result<DatasetInfo> {
        let store = self.open(name)?;
        let corpus = store.corpus_info()?;
        let stats = store.stats()?;

        Ok(DatasetInfo {
            name: corpus.name,
            language: corpus.language,
            description: corpus.description,
            tokens: corpus.token_count,
            documents: stats.documents,
            sentences: stats.sentences,
            created_at: corpus.created_at,
        })
    }

    /// A frequency list.
    pub fn frequency(&self, name: &str, by: By, limit: usize) -> Result<FreqResponse> {
        let store = self.open(name)?;
        let tokens = store.token_count()?;
        let rows = query::frequency(&store, by, limit)?;
        Ok(FreqResponse::new(name, by.label(), tokens, &rows))
    }

    /// A pattern search.
    pub fn query(&self, name: &str, options: &search::SearchOptions) -> Result<SearchResponse> {
        let store = self.open(name)?;
        let result = search::search(&store, options)?;
        Ok(SearchResponse::from_result(name, &result))
    }

    /// Import local files into a dataset.
    pub fn import(
        &self,
        options: &ingest::ImportOptions,
        path: &Path,
    ) -> Result<ingest::ImportStats> {
        let db_path = self.registry.target_path(&options.dataset);
        let mut store = Store::open(&db_path)?;
        ingest::import_path(&mut store, options, path)
    }

    /// Download a catalog entry and import it into a dataset.
    pub fn add(&self, dataset: &str, options: &AddOptions) -> Result<AddOutcome> {
        let directory = options
            .to
            .clone()
            .unwrap_or_else(|| self.registry.download_dir(&options.source));

        let manifest = slovo_fetch::fetch(&FetchOptions {
            catalog: options.catalog.clone(),
            id: options.source.clone(),
            to: directory.clone(),
            force: options.force,
        })
        .map_err(|error| Error::Other(format!("could not fetch `{}`: {error}", options.source)))?;

        // The manifest fills in anything the caller did not override.
        let import_options = ingest::ImportOptions {
            dataset: dataset.to_string(),
            language: options
                .language
                .clone()
                .or_else(|| manifest.language.clone()),
            description: options
                .description
                .clone()
                .or_else(|| Some(manifest.name.clone())),
            title: options.title.clone(),
            source: manifest.source.clone(),
            license: manifest.license.clone(),
            version: manifest.version.clone(),
        };

        let stats = self.import(&import_options, &directory)?;

        Ok(AddOutcome {
            manifest,
            stats,
            directory,
        })
    }
}
