use std::path::PathBuf;

/// Everything that can go wrong while annotating.
#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The external tagger command was missing or exited non-zero.
    Tagger { command: String, message: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io { path, source } => {
                write!(f, "could not read `{}`: {source}", path.display())
            }
            Error::Tagger { command, message } => {
                write!(f, "tagger `{command}` failed: {message}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            Error::Tagger { .. } => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
