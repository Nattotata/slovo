//! Source documents.
//!
//! A hit points at a document id; this resolves it to the whole text, so a
//! concordance line can be read in full context (`slovo doc`). The text is
//! whatever the importer stored: for CoNLL-U that is the `# text` of each
//! sentence, which is the source as the corpus saw it.

use rusqlite::params;

use crate::error::{Error, Result};
use crate::store::Store;

/// One document's metadata and its sentences, in reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub id: i64,
    pub title: Option<String>,
    pub source_url: Option<String>,
    pub license: Option<String>,
    /// A path, or `<file>#<newdoc>` when the file was split by document.
    pub external_id: Option<String>,
    pub sentences: Vec<DocumentSentence>,
}

/// One sentence of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSentence {
    pub id: i64,
    /// The `# sent_id`, when the source had one.
    pub sent_id: Option<String>,
    pub text: Option<String>,
}

/// Fetch a document by id, with all of its sentences.
pub fn document(store: &Store, document_id: i64) -> Result<Document> {
    let header = store
        .conn()
        .query_row(
            "SELECT id, title, source_url, license, external_id
             FROM document WHERE id = ?1",
            params![document_id],
            |r| {
                Ok(Document {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    source_url: r.get(2)?,
                    license: r.get(3)?,
                    external_id: r.get(4)?,
                    sentences: Vec::new(),
                })
            },
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::Other(format!("document {document_id} not found"))
            }
            other => Error::Db(other),
        })?;

    let mut stmt = store.conn().prepare(
        "SELECT id, external_id, text FROM sentence
         WHERE document_id = ?1 ORDER BY position",
    )?;
    let rows = stmt.query_map(params![document_id], |r| {
        Ok(DocumentSentence {
            id: r.get(0)?,
            sent_id: r.get(1)?,
            text: r.get(2)?,
        })
    })?;

    let mut doc = header;
    for row in rows {
        doc.sentences.push(row?);
    }
    Ok(doc)
}

/// The documents in a dataset, newest first is not meaningful, so by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSummary {
    pub id: i64,
    pub title: Option<String>,
    pub source_url: Option<String>,
    pub sentences: u64,
    pub tokens: u64,
}

/// List documents, with sentence and token counts.
pub fn documents(store: &Store, limit: usize) -> Result<Vec<DocumentSummary>> {
    let mut stmt = store.conn().prepare(
        "SELECT d.id, d.title, d.source_url,
                (SELECT COUNT(*) FROM sentence s WHERE s.document_id = d.id),
                (SELECT COUNT(*) FROM token t
                   JOIN sentence s ON s.id = t.sentence_id
                  WHERE s.document_id = d.id)
         FROM document d
         ORDER BY d.id
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |r| {
        Ok(DocumentSummary {
            id: r.get(0)?,
            title: r.get(1)?,
            source_url: r.get(2)?,
            sentences: r.get::<_, i64>(3)? as u64,
            tokens: r.get::<_, i64>(4)? as u64,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
