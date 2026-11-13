//! The dataset boundary.
//!
//! A dataset is a directory of source files plus a `manifest.json`.
//! The app imports that directory; it never knows which corpus it came from.
//!
//! A *catalog* is a separate JSON file listing datasets that can be
//! downloaded. It is a plain data file, not code: it can be edited, swapped,
//! or pointed at a different location without rebuilding the app.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file name of the manifest inside a dataset directory.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Errors raised while reading or writing a manifest or catalog.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("malformed JSON in `{path}`: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("dataset `{0}` not found in the catalog")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// One file in a dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    /// File name inside the dataset directory.
    pub name: String,
    /// Where to download it from. `None` for a local-only dataset.
    #[serde(default)]
    pub url: Option<String>,
    /// Lower-case hex SHA-256. Optional, but recommended for downloads.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// A dataset directory's manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Human-readable name, e.g. `UD English EWT`.
    pub name: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub files: Vec<File>,
}

/// A catalog entry: a dataset plus where to fetch it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    /// Stable identifier, e.g. `ud/UD_English-EWT`.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default)]
    pub files: Vec<File>,
}

/// The catalog: a list of datasets and where to fetch them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub version: u32,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub datasets: Vec<Dataset>,
}

fn default_format() -> String {
    "conllu".to_string()
}

impl Manifest {
    /// Read a manifest from a file.
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_slice(&bytes).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Write a manifest to a file.
    pub fn write(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })?;
        std::fs::write(path, bytes).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Read `manifest.json` from a directory, if it is there.
    pub fn read_dir(dir: &Path) -> Result<Option<Self>> {
        let path = dir.join(MANIFEST_FILE);
        if path.is_file() {
            Ok(Some(Self::read(&path)?))
        } else {
            Ok(None)
        }
    }
}

impl Dataset {
    /// Turn a catalog entry into the manifest written into the dataset directory.
    pub fn to_manifest(&self) -> Manifest {
        Manifest {
            name: self.name.clone(),
            language: self.language.clone(),
            license: self.license.clone(),
            source: self.homepage.clone(),
            version: self.version.clone(),
            format: self.format.clone(),
            files: self.files.clone(),
        }
    }
}

impl Catalog {
    /// Read a catalog from a local file.
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_slice(&bytes).map_err(|source| Error::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Parse a catalog from JSON bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(|source| Error::Json {
            path: PathBuf::from("<catalog>"),
            source,
        })
    }

    /// Look up a dataset by id.
    pub fn find(&self, id: &str) -> Result<&Dataset> {
        self.datasets
            .iter()
            .find(|dataset| dataset.id == id)
            .ok_or_else(|| Error::NotFound(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_catalog() {
        let catalog = Catalog::parse(
            br#"{
                "version": 1,
                "datasets": [
                    {
                        "id": "ud/UD_English-EWT",
                        "name": "UD English EWT",
                        "language": "en",
                        "license": "CC BY-SA 4.0",
                        "files": [
                            {"name": "en_ewt-ud-train.conllu", "url": "https://example.com/x", "sha256": "abc"}
                        ]
                    }
                ]
            }"#,
        )
        .unwrap();

        let dataset = catalog.find("ud/UD_English-EWT").unwrap();
        assert_eq!(dataset.name, "UD English EWT");
        assert_eq!(dataset.format, "conllu");
        assert_eq!(dataset.files[0].name, "en_ewt-ud-train.conllu");
    }

    #[test]
    fn missing_dataset_is_an_error() {
        let catalog = Catalog::parse(br#"{"version": 1, "datasets": []}"#).unwrap();
        assert!(matches!(catalog.find("nope"), Err(Error::NotFound(_))));
    }
}
