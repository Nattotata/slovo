//! Ingestion: parse a source file (or directory) and load it into a dataset.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::params;

use slovo_dataset::Manifest;

use crate::conllu;
use crate::error::{Error, Result};
use crate::store::Store;

/// How to build a dataset.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub dataset: String,
    pub language: Option<String>,
    pub description: Option<String>,
    /// Title for the document; defaults to the file stem.
    pub title: Option<String>,
    /// Provenance: where the source came from.
    pub source: Option<String>,
    /// Provenance: the source's license.
    pub license: Option<String>,
    /// Provenance: the source's version.
    pub version: Option<String>,
}

impl ImportOptions {
    pub fn new(dataset: impl Into<String>) -> Self {
        Self {
            dataset: dataset.into(),
            language: None,
            description: None,
            title: None,
            source: None,
            license: None,
            version: None,
        }
    }
}

/// What an import actually loaded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportStats {
    pub documents: u64,
    pub sentences: u64,
    pub tokens: u64,
    /// Files already present in the dataset, and therefore not imported again.
    pub skipped: u64,
}

/// Import a `.conllu` file, or every `.conllu` file in a directory.
///
/// If the directory contains a `manifest.json`, its metadata fills in the
/// options that were not set explicitly.
pub fn import_path(store: &mut Store, opts: &ImportOptions, path: &Path) -> Result<ImportStats> {
    let mut opts = opts.clone();
    if path.is_dir() {
        if let Some(manifest) = Manifest::read_dir(path)? {
            opts.language = opts.language.or(manifest.language);
            opts.source = opts.source.or(manifest.source);
            opts.license = opts.license.or(manifest.license);
            opts.version = opts.version.or(manifest.version);
            opts.description = opts.description.or(Some(manifest.name));
        }
    }

    let corpus_id = store.upsert_corpus(
        &opts.dataset,
        opts.language.as_deref(),
        opts.description.as_deref(),
    )?;

    let files = collect_files(path)?;
    if files.is_empty() {
        return Err(Error::Other(format!(
            "no `.conllu` files found at `{}`",
            path.display()
        )));
    }

    let mut stats = ImportStats::default();
    let existing = imported_external_ids(store, corpus_id)?;

    for file in &files {
        let external_id = file.to_string_lossy().to_string();
        if existing.contains(&external_id) {
            stats.skipped += 1;
            continue;
        }

        let (sentences, tokens) = import_file(store, corpus_id, &opts, file)?;
        stats.documents += 1;
        stats.sentences += sentences;
        stats.tokens += tokens;
    }

    store.add_token_count(corpus_id, stats.tokens as i64)?;
    Ok(stats)
}

/// External ids of the documents already in this dataset, so re-importing the
/// same files is a no-op rather than silently doubling the corpus.
fn imported_external_ids(store: &Store, corpus_id: i64) -> Result<HashSet<String>> {
    let mut stmt = store
        .conn()
        .prepare("SELECT external_id FROM document WHERE corpus_id = ?1")?;
    let rows = stmt.query_map(params![corpus_id], |row| row.get::<_, Option<String>>(0))?;

    let mut ids = HashSet::new();
    for row in rows {
        if let Some(id) = row? {
            ids.insert(id);
        }
    }
    Ok(ids)
}

fn collect_files(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.is_dir() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let candidate = entry.path();
        let is_conllu = candidate
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("conllu"))
            .unwrap_or(false);
        if candidate.is_file() && is_conllu {
            files.push(candidate);
        }
    }
    files.sort();
    Ok(files)
}

/// Insert one file as a single document. Returns `(sentences, tokens)`.
fn import_file(
    store: &mut Store,
    corpus_id: i64,
    opts: &ImportOptions,
    path: &Path,
) -> Result<(u64, u64)> {
    let sentences = conllu::parse_file(path)?;

    let title = opts.title.clone().or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    });
    let external_id = path.to_string_lossy().to_string();
    let meta = opts
        .version
        .as_ref()
        .map(|version| serde_json::json!({ "version": version }).to_string());

    let tx = store.conn_mut().transaction()?;

    let document_id: i64 = tx.query_row(
        "INSERT INTO document (corpus_id, external_id, title, source_url, license, meta)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         RETURNING id",
        params![
            corpus_id,
            external_id,
            title.as_deref(),
            opts.source.as_deref(),
            opts.license.as_deref(),
            meta.as_deref(),
        ],
        |row| row.get(0),
    )?;

    let mut sentence_stmt = tx.prepare(
        "INSERT INTO sentence (document_id, position, text) VALUES (?1, ?2, ?3) RETURNING id",
    )?;
    let mut token_stmt = tx.prepare(
        "INSERT INTO token
           (sentence_id, position, form, lower, lemma, upos, xpos, feats, head, deprel)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;

    let mut sentence_count = 0u64;
    let mut token_count = 0u64;

    for (i, sentence) in sentences.iter().enumerate() {
        let sentence_id: i64 = sentence_stmt.query_row(
            params![document_id, i as i64, sentence.text.as_deref()],
            |row| row.get(0),
        )?;

        for (j, token) in sentence.tokens.iter().enumerate() {
            token_stmt.execute(params![
                sentence_id,
                (j + 1) as i64,
                &token.form,
                token.form.to_lowercase(),
                token.lemma.as_deref(),
                token.upos.as_deref(),
                token.xpos.as_deref(),
                token.feats.as_deref(),
                token.head,
                token.deprel.as_deref(),
            ])?;
        }

        sentence_count += 1;
        token_count += sentence.tokens.len() as u64;
    }

    drop(token_stmt);
    drop(sentence_stmt);
    tx.commit()?;

    Ok((sentence_count, token_count))
}
