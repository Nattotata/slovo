use std::path::PathBuf;

/// Everything that can go wrong inside the engine.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("could not read `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("malformed CoNLL-U in `{path}` at line {line}: {message}")]
    Conllu {
        path: PathBuf,
        line: usize,
        message: String,
    },

    #[error("dataset `{0}` not found")]
    DatasetNotFound(String),

    #[error("dataset error: {0}")]
    Dataset(#[from] slovo_dataset::Error),

    #[error("bad query: {0}")]
    Query(crate::query_lang::QueryError),

    #[error("{0}")]
    Other(String),
}

impl From<crate::query_lang::QueryError> for Error {
    fn from(error: crate::query_lang::QueryError) -> Self {
        Error::Query(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
