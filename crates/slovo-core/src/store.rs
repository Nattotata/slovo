//! Thin wrapper around a SQLite connection.
//!
//! One file per dataset. `Store` owns the connection and exposes the small
//! set of operations the frontends need.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use crate::error::{Error, Result};
use crate::schema::{INIT_SQL, MIGRATIONS, POST_MIGRATION_SQL};

pub struct Store {
    conn: Connection,
    path: PathBuf,
}

/// Row counts for one dataset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub documents: u64,
    pub sentences: u64,
    pub tokens: u64,
}

/// Metadata about the dataset (there is exactly one `corpus` row per file).
#[derive(Debug, Clone)]
pub struct CorpusInfo {
    pub name: String,
    pub language: Option<String>,
    pub description: Option<String>,
    pub token_count: u64,
    pub created_at: String,
}

impl Store {
    /// Open a dataset, creating the file and schema if needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch(INIT_SQL)?;
        migrate(&conn)?;
        conn.execute_batch(POST_MIGRATION_SQL)?;
        Ok(Self { conn, path })
    }

    /// Open an existing dataset, failing if it is not there.
    pub fn open_existing(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::DatasetNotFound(path.display().to_string()));
        }
        Self::open(path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub(crate) fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Insert the dataset row if absent, and return its id.
    pub fn upsert_corpus(
        &self,
        name: &str,
        language: Option<&str>,
        description: Option<&str>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO corpus (name, language, description, created_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(name) DO UPDATE SET
               language    = COALESCE(excluded.language, corpus.language),
               description = COALESCE(excluded.description, corpus.description)",
            params![name, language, description],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM corpus WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    /// Add to the running token count on the dataset row.
    pub fn add_token_count(&self, corpus_id: i64, delta: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE corpus SET token_count = token_count + ?1 WHERE id = ?2",
            params![delta, corpus_id],
        )?;
        Ok(())
    }

    pub fn corpus_info(&self) -> Result<CorpusInfo> {
        self.conn
            .query_row(
                "SELECT name, language, description, token_count, created_at
                 FROM corpus ORDER BY id LIMIT 1",
                [],
                |r| {
                    Ok(CorpusInfo {
                        name: r.get(0)?,
                        language: r.get(1)?,
                        description: r.get(2)?,
                        token_count: r.get::<_, i64>(3)? as u64,
                        created_at: r.get(4)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::Other("dataset is empty".into()),
                other => Error::Db(other),
            })
    }

    pub fn token_count(&self) -> Result<u64> {
        let n: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(token_count), 0) FROM corpus",
            [],
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }

    pub fn stats(&self) -> Result<Stats> {
        let count = |table: &str| -> Result<u64> {
            let sql = format!("SELECT COUNT(*) FROM {table}");
            let n: i64 = self.conn.query_row(&sql, [], |r| r.get(0))?;
            Ok(n as u64)
        };
        Ok(Stats {
            documents: count("document")?,
            sentences: count("sentence")?,
            tokens: count("token")?,
        })
    }
}

/// Add any columns a dataset file predates. No-op on a fresh file, where
/// `INIT_SQL` already created every column.
fn migrate(conn: &Connection) -> Result<()> {
    for (table, column, definition) in MIGRATIONS {
        if !column_exists(conn, table, column)? {
            conn.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {column} {definition}"
            ))?;
        }
    }
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_a_pre_v3_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");

        // Build the v1 tables, without the later columns.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE sentence (
                   id          INTEGER PRIMARY KEY,
                   document_id INTEGER NOT NULL,
                   position    INTEGER NOT NULL,
                   text        TEXT
                 );
                 CREATE TABLE token (
                   id          INTEGER PRIMARY KEY,
                   sentence_id INTEGER NOT NULL,
                   position    INTEGER NOT NULL,
                   form        TEXT NOT NULL,
                   lower       TEXT NOT NULL,
                   lemma       TEXT,
                   upos        TEXT,
                   xpos        TEXT,
                   feats       TEXT,
                   head        INTEGER,
                   deprel      TEXT
                 );",
            )
            .unwrap();
        }

        // Opening it migrates in place.
        let store = Store::open(&path).unwrap();
        assert!(column_exists(store.conn(), "sentence", "external_id").unwrap());
        assert!(column_exists(store.conn(), "sentence", "newdoc_id").unwrap());
        assert!(column_exists(store.conn(), "token", "lemma_override").unwrap());
        assert!(column_exists(store.conn(), "token", "upos_override").unwrap());
    }

    #[test]
    fn a_fresh_file_has_every_column() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("fresh.db")).unwrap();
        for (table, column, _) in MIGRATIONS {
            assert!(
                column_exists(store.conn(), table, column).unwrap(),
                "{table}.{column} missing"
            );
        }
    }
}
