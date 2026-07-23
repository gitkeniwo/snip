use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::domain::{FORMAT_NAME, LibraryManifest, SCHEMA_VERSION, TagRegistry};
use crate::error::{Result, SnipError};

use super::io::{atomic_write, validate_schema};
use super::paths::now_rfc3339;

pub(crate) const LIBRARY_MANIFEST: &str = "snip.toml";

#[derive(Clone, Debug)]
pub struct Library {
    pub(crate) root: PathBuf,
    pub(crate) manifest: LibraryManifest,
}

pub struct LibraryLock {
    file: File,
}

impl Drop for LibraryLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl Library {
    pub fn init(path: &Path, name: Option<&str>) -> Result<Self> {
        if path.join(LIBRARY_MANIFEST).exists() {
            return Err(SnipError::conflict(format!(
                "library already exists: {}",
                path.display()
            )));
        }
        fs::create_dir_all(path)
            .map_err(|error| SnipError::io(format!("cannot create {}: {error}", path.display())))?;
        for directory in [
            "snippets",
            "trash",
            ".snip/cache",
            ".snip/locks",
            ".snip/transactions",
        ] {
            fs::create_dir_all(path.join(directory))
                .map_err(|error| SnipError::io(format!("cannot create {directory}: {error}")))?;
        }
        let inferred_name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("Snippets");
        let manifest = LibraryManifest {
            format: FORMAT_NAME.to_owned(),
            schema_version: SCHEMA_VERSION,
            id: Uuid::new_v4(),
            name: name.unwrap_or(inferred_name).to_owned(),
            created_at: now_rfc3339()?,
            extra: toml::Table::new(),
        };
        let data = toml::to_string_pretty(&manifest)?;
        atomic_write(&path.join(LIBRARY_MANIFEST), data.as_bytes())?;
        let tags = TagRegistry {
            schema_version: SCHEMA_VERSION,
            tags: Vec::new(),
            extra: toml::Table::new(),
        };
        atomic_write(
            &path.join("tags.toml"),
            toml::to_string_pretty(&tags)?.as_bytes(),
        )?;
        let ignore = ".snip/cache/\n.snip/locks/\n.snip/transactions/\n.DS_Store\n";
        atomic_write(&path.join(".gitignore"), ignore.as_bytes())?;
        Self::open(path)
    }

    pub fn discover(explicit: Option<&Path>, configured: Option<&Path>) -> Result<PathBuf> {
        if let Some(path) = explicit {
            return Ok(path.to_path_buf());
        }
        if let Some(path) = std::env::var_os("SNIP_LIBRARY") {
            return Ok(PathBuf::from(path));
        }
        let current = std::env::current_dir()
            .map_err(|error| SnipError::io(format!("cannot get current directory: {error}")))?;
        for candidate in current.ancestors() {
            if candidate.join(LIBRARY_MANIFEST).is_file() {
                return Ok(candidate.to_path_buf());
            }
        }
        if let Some(path) = configured {
            return Ok(path.to_path_buf());
        }
        Err(SnipError::not_found(
            "no snip library found; pass --library, set SNIP_LIBRARY, run inside a library, or configure default_library",
        ))
    }

    pub fn open(path: &Path) -> Result<Self> {
        let root = fs::canonicalize(path).map_err(|error| {
            SnipError::not_found(format!(
                "library does not exist: {}: {error}",
                path.display()
            ))
        })?;
        let manifest_path = root.join(LIBRARY_MANIFEST);
        let manifest_text = fs::read_to_string(&manifest_path).map_err(|error| {
            SnipError::validation(format!("cannot read {}: {error}", manifest_path.display()))
        })?;
        let manifest: LibraryManifest = toml::from_str(&manifest_text).map_err(|error| {
            SnipError::validation(format!("cannot parse {}: {error}", manifest_path.display()))
        })?;
        if manifest.format != FORMAT_NAME {
            return Err(SnipError::validation(format!(
                "{} has unsupported format {:?}",
                manifest_path.display(),
                manifest.format
            )));
        }
        validate_schema(manifest.schema_version, &manifest_path)?;
        for required in ["snippets", "trash", ".snip"] {
            if !root.join(required).is_dir() {
                return Err(SnipError::validation(format!(
                    "library is missing directory: {}",
                    root.join(required).display()
                )));
            }
        }
        Ok(Self { root, manifest })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &LibraryManifest {
        &self.manifest
    }

    pub fn snippets_dir(&self) -> PathBuf {
        self.root.join("snippets")
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.root.join("trash")
    }

    pub fn transactions_dir(&self) -> PathBuf {
        self.root.join(".snip/transactions")
    }

    pub fn lock(&self) -> Result<LibraryLock> {
        let lock_path = self.root.join(".snip/locks/library.lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|error| {
                SnipError::io(format!("cannot open {}: {error}", lock_path.display()))
            })?;
        file.try_lock_exclusive().map_err(|error| {
            SnipError::conflict(format!("library is locked by another process: {error}"))
        })?;
        Ok(LibraryLock { file })
    }
}
