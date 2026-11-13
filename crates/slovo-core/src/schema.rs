//! The on-disk layout. One database file per dataset.
//!
//! The `token` table is deliberately the CWB "positional attribute" model:
//! a single ordered stream of tokens where each row carries the word form and
//! its annotations. `token.id` is the corpus position.

/// Bumped whenever `INIT_SQL` changes in a way that needs a migration.
pub const SCHEMA_VERSION: u32 = 1;

pub const INIT_SQL: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS corpus (
  id          INTEGER PRIMARY KEY,
  name        TEXT UNIQUE NOT NULL,
  language    TEXT,
  description TEXT,
  token_count INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS document (
  id          INTEGER PRIMARY KEY,
  corpus_id   INTEGER NOT NULL REFERENCES corpus(id) ON DELETE CASCADE,
  external_id TEXT,
  title       TEXT,
  source_url  TEXT,
  license     TEXT,
  meta        TEXT
);
CREATE INDEX IF NOT EXISTS document_corpus_idx ON document(corpus_id);

CREATE TABLE IF NOT EXISTS sentence (
  id          INTEGER PRIMARY KEY,
  document_id INTEGER NOT NULL REFERENCES document(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  text        TEXT
);
CREATE INDEX IF NOT EXISTS sentence_document_idx ON sentence(document_id);

CREATE TABLE IF NOT EXISTS token (
  id          INTEGER PRIMARY KEY,
  sentence_id INTEGER NOT NULL REFERENCES sentence(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  form        TEXT NOT NULL,
  lower       TEXT NOT NULL,
  lemma       TEXT,
  upos        TEXT,
  xpos        TEXT,
  feats       TEXT,
  head        INTEGER,
  deprel      TEXT
);
CREATE INDEX IF NOT EXISTS token_sentence_idx ON token(sentence_id);
CREATE INDEX IF NOT EXISTS token_lower_idx    ON token(lower);
CREATE INDEX IF NOT EXISTS token_lemma_idx    ON token(lemma);
CREATE INDEX IF NOT EXISTS token_upos_idx     ON token(upos);
"#;
