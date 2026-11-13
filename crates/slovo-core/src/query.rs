//! Query layer. Frequency lists for now; concordance, n-grams and collocation
//! statistics land here later.

use rusqlite::params;

use crate::error::Result;
use crate::store::Store;

/// Which column to group a frequency list by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// Lower-cased surface form.
    Word,
    /// Exact surface form, case-sensitive.
    Form,
    Lemma,
    Upos,
    Xpos,
}

impl By {
    fn column(self) -> &'static str {
        match self {
            By::Word => "lower",
            By::Form => "form",
            By::Lemma => "lemma",
            By::Upos => "upos",
            By::Xpos => "xpos",
        }
    }

    /// Short human-facing name, used in output headers.
    pub fn label(self) -> &'static str {
        match self {
            By::Word => "word",
            By::Form => "form",
            By::Lemma => "lemma",
            By::Upos => "upos",
            By::Xpos => "xpos",
        }
    }
}

/// One line of a frequency list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreqRow {
    pub value: String,
    pub count: u64,
}

/// The `limit` most frequent values in the dataset.
pub fn frequency(store: &Store, by: By, limit: usize) -> Result<Vec<FreqRow>> {
    let column = by.column();
    let sql = format!(
        "SELECT {column} AS value, COUNT(*) AS n
         FROM token
         WHERE {column} IS NOT NULL AND {column} <> ''
         GROUP BY {column}
         ORDER BY n DESC, value ASC
         LIMIT ?1"
    );

    let mut stmt = store.conn().prepare(&sql)?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok(FreqRow {
            value: row.get(0)?,
            count: row.get::<_, i64>(1)? as u64,
        })
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Normalise a raw count per million tokens.
pub fn per_million(count: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        count as f64 / total as f64 * 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_million_is_stable_at_zero_total() {
        assert_eq!(per_million(10, 0), 0.0);
        assert_eq!(per_million(1, 1_000_000), 1.0);
    }
}
