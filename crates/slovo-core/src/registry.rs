//! Dataset registry: maps a dataset name to a database file.
//!
//! Layout: `<root>/datasets/<name>.db`. `SLOVO_DATA_DIR` sets the root;
//! otherwise we use the platform data dir (`~/.local/share/slovo` on Linux,
//! `~/Library/Application Support/slovo` on macOS, `%APPDATA%\slovo` on
//! Windows).
//!
//! [`Registry`] carries the root explicitly, so a caller (a test, a server
//! with several corpora roots) can use something other than the environment.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub const ENV_DATA_DIR: &str = "SLOVO_DATA_DIR";

/// A directory holding datasets.
#[derive(Debug, Clone)]
pub struct Registry {
    root: PathBuf,
}

impl Registry {
    /// The registry selected by `SLOVO_DATA_DIR` or the platform data dir.
    pub fn from_env() -> Self {
        Self::at(data_dir())
    }

    /// A registry rooted somewhere specific.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn datasets_dir(&self) -> PathBuf {
        self.root.join("datasets")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.root.join("downloads")
    }

    /// Where a downloaded dataset is unpacked: `<root>/downloads/<id>`.
    pub fn download_dir(&self, id: &str) -> PathBuf {
        self.downloads_dir().join(id.replace('/', "-"))
    }

    pub fn dataset_path(&self, name: &str) -> PathBuf {
        self.datasets_dir().join(format!("{name}.db"))
    }

    /// Turn a `-d` value into a database path.
    ///
    /// Anything that looks like a path (contains a separator, or ends in
    /// `.db`) is used verbatim; otherwise it is a dataset name looked up in
    /// the store.
    pub fn resolve(&self, name: &str) -> Result<PathBuf> {
        if is_path(name) {
            return Ok(PathBuf::from(name));
        }
        let path = self.dataset_path(name);
        if path.exists() {
            Ok(path)
        } else {
            Err(Error::DatasetNotFound(name.to_string()))
        }
    }

    /// Path for a dataset being created, whether by name or by explicit path.
    pub fn target_path(&self, name: &str) -> PathBuf {
        if is_path(name) {
            PathBuf::from(name)
        } else {
            self.dataset_path(name)
        }
    }

    pub fn exists(&self, name: &str) -> bool {
        self.dataset_path(name).exists()
    }

    /// Names of every installed dataset, sorted.
    pub fn list(&self) -> Result<Vec<String>> {
        let dir = self.datasets_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(|source| Error::Io {
            path: dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            if let (Some(stem), Some("db")) = (
                path.file_stem().and_then(|s| s.to_str()),
                path.extension().and_then(|s| s.to_str()),
            ) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }
}

fn is_path(name: &str) -> bool {
    name.ends_with(".db") || name.contains('/')
}

/// The base data directory, before the `datasets/` part.
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(ENV_DATA_DIR) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    match dirs::data_dir() {
        Some(dir) => dir.join("slovo"),
        None => PathBuf::from(".slovo"),
    }
}
