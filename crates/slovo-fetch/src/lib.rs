//! Downloader for Slovo datasets.
//!
//! This crate knows about catalogs, HTTP and checksums. It does not know
//! about the app's database: it only writes a dataset directory, which the
//! app imports later. That is the boundary.

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use slovo_dataset::{Catalog, Dataset, Manifest, MANIFEST_FILE};

/// Default catalog location, relative to the working directory.
pub const DEFAULT_CATALOG: &str = "catalog.json";

/// Environment variable to override the catalog location.
pub const ENV_CATALOG: &str = "SLOVO_CATALOG";

/// Errors raised while downloading.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http error: {0}")]
    Http(#[from] ureq::Error),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("dataset error: {0}")]
    Dataset(#[from] slovo_dataset::Error),

    #[error("checksum mismatch for `{name}`: expected {expected}, got {actual}")]
    Checksum {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Options for a fetch.
#[derive(Debug, Clone)]
pub struct FetchOptions {
    /// Catalog URL or local path. `None` uses [`DEFAULT_CATALOG`] or
    /// [`ENV_CATALOG`].
    pub catalog: Option<String>,
    pub id: String,
    pub to: PathBuf,
    pub force: bool,
}

/// Download a dataset from a catalog into a directory, and write its manifest.
///
/// Existing files are skipped when the checksum matches. A mismatched or
/// missing file is downloaded again unless `force` is set.
pub fn fetch(opts: &FetchOptions) -> Result<Manifest> {
    let catalog = load_catalog(opts.catalog.as_deref())?;
    let dataset = catalog.find(&opts.id)?;
    let manifest = dataset.to_manifest();

    std::fs::create_dir_all(&opts.to)?;
    download_dataset(dataset, &opts.to, opts.force)?;
    manifest.write(&opts.to.join(MANIFEST_FILE))?;

    Ok(manifest)
}

/// Load a catalog from a local path or a URL.
pub fn load_catalog(source: Option<&str>) -> Result<Catalog> {
    let source = source
        .map(str::to_string)
        .or_else(|| std::env::var(ENV_CATALOG).ok())
        .unwrap_or_else(|| DEFAULT_CATALOG.to_string());

    if source.starts_with("http://") || source.starts_with("https://") {
        let bytes = fetch_bytes(&source)?;
        Ok(Catalog::parse(&bytes)?)
    } else {
        Ok(Catalog::read(Path::new(&source))?)
    }
}

fn download_dataset(dataset: &Dataset, dir: &Path, force: bool) -> Result<()> {
    for file in &dataset.files {
        let url = file
            .url
            .as_deref()
            .ok_or_else(|| Error::Other(format!("no URL for `{}`", file.name)))?;
        let path = dir.join(&file.name);

        if path.exists() && !force {
            if let Some(expected) = &file.sha256 {
                if verify(&path, expected).is_ok() {
                    continue;
                }
            }
        }

        let bytes = fetch_bytes(url)?;
        std::fs::write(&path, &bytes)?;

        if let Some(expected) = &file.sha256 {
            verify(&path, expected)?;
        }
    }
    Ok(())
}

fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url).call()?;
    let mut bytes = Vec::new();
    response.body_mut().as_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Verify a file's SHA-256 against a lower-case hex digest.
fn verify(path: &Path, expected: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let actual = hex(&hasher.finalize());
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(Error::Checksum {
            name: path.display().to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_a_known_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x");
        std::fs::write(&path, b"hello").unwrap();

        // SHA-256("hello")
        verify(
            &path,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        )
        .unwrap();
    }

    #[test]
    fn rejects_a_bad_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x");
        std::fs::write(&path, b"hello").unwrap();

        assert!(matches!(
            verify(
                &path,
                "0000000000000000000000000000000000000000000000000000000000000000"
            ),
            Err(Error::Checksum { .. })
        ));
    }
}
