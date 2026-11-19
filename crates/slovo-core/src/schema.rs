//! The on-disk layout. One database file per dataset.
//!
//! The `token` table is deliberately the CWB "positional attribute" model:
//! a single ordered stream of tokens where each row carries the word form and
//! its annotations. `token.id` is the corpus position.

/// Bumped whenever `INIT_SQL` changes in a way that needs a migration.
pub const SCHEMA_VERSION: u32 = 4;

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
  text        TEXT,
  external_id TEXT,
  newdoc_id   TEXT
);
CREATE INDEX IF NOT EXISTS sentence_document_idx ON sentence(document_id);

CREATE TABLE IF NOT EXISTS token (
  id              INTEGER PRIMARY KEY,
  sentence_id     INTEGER NOT NULL REFERENCES sentence(id) ON DELETE CASCADE,
  position        INTEGER NOT NULL,
  form            TEXT NOT NULL,
  lower           TEXT NOT NULL,
  lemma           TEXT,
  upos            TEXT,
  xpos            TEXT,
  feats           TEXT,
  head            INTEGER,
  deprel          TEXT,
  lemma_override  TEXT,
  upos_override   TEXT,
  weight          REAL NOT NULL DEFAULT 1.0
);
CREATE INDEX IF NOT EXISTS token_sentence_idx ON token(sentence_id);
CREATE INDEX IF NOT EXISTS token_lower_idx    ON token(lower);
CREATE INDEX IF NOT EXISTS token_lemma_idx    ON token(lemma);
CREATE INDEX IF NOT EXISTS token_upos_idx     ON token(upos);
"#;

/// Columns added to existing tables after their first release.
///
/// SQLite's `CREATE TABLE IF NOT EXISTS` will not add a column to a table that
/// already exists, so an older dataset file is upgraded by adding any column
/// that is missing. Each entry is `(table, column, definition)`.
pub const MIGRATIONS: &[(&str, &str, &str)] = &[
    ("sentence", "external_id", "TEXT"),
    ("sentence", "newdoc_id", "TEXT"),
    ("token", "lemma_override", "TEXT"),
    ("token", "upos_override", "TEXT"),
    ("token", "weight", "REAL NOT NULL DEFAULT 1.0"),
];

/// Indexes that depend on migrated columns, so they are created *after* the
/// migration runs. An index in `INIT_SQL` would fail on an old file, where the
/// column does not exist yet.
pub const POST_MIGRATION_SQL: &str = r#"
CREATE INDEX IF NOT EXISTS token_lemma_override_idx ON token(lemma_override);
"#;
