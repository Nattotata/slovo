//! Dataset registry: maps a dataset name to a database file.
//!
//! Layout: `<data_dir>/datasets/<name>.db`. `SLOVO_DATA_DIR` overrides the
//! base directory; otherwise we use the platform data dir
//! (`~/.local/share/slovo` on Linux, `~/Library/Application Support/slovo`
//! on macOS, `%APPDATA%\slovo` on Windows).

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub const ENV_DATA_DIR: &str = "SLOVO_DATA_DIR";

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

pub fn datasets_dir() -> PathBuf {
    data_dir().join("datasets")
}

pub fn dataset_path(name: &str) -> PathBuf {
    datasets_dir().join(format!("{name}.db"))
}

/// Turn a `-d` value into a database path.
///
/// Anything that looks like a path (contains a separator, or ends in `.db`)
/// is used verbatim; otherwise it is a dataset name looked up in the store.
/// Missing named datasets are an error; missing explicit paths are allowed
/// (so `import` can create them).
pub fn resolve(name: &str) -> Result<PathBuf> {
    if name.ends_with(".db") || name.contains('/') {
        return Ok(PathBuf::from(name));
    }
    let path = dataset_path(name);
    if path.exists() {
        Ok(path)
    } else {
        Err(Error::DatasetNotFound(name.to_string()))
    }
}

pub fn exists(name: &str) -> bool {
    dataset_path(name).exists()
}

/// Names of every installed dataset, sorted.
pub fn list() -> Result<Vec<String>> {
    let dir = datasets_dir();
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

/// Path for a dataset being created by `import`.
pub fn target_path(name: &str) -> PathBuf {
    if name.ends_with(".db") || name.contains('/') {
        PathBuf::from(name)
    } else {
        dataset_path(name)
    }
}

/// Whether the resolved path points at an existing file.
pub fn is_file(path: &Path) -> bool {
    path.is_file()
}
