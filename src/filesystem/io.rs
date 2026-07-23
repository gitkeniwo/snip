use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

use crate::domain::{FragmentManifest, SCHEMA_VERSION, SnippetManifest};
use crate::error::{Result, SnipError};

use super::paths::{normalize_tags, resolve_managed_path};

pub fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| SnipError::validation(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|error| SnipError::io(format!("cannot create {}: {error}", parent.display())))?;
    let permissions = fs::metadata(path).ok().map(|value| value.permissions());
    let mut temp = NamedTempFile::new_in(parent).map_err(|error| {
        SnipError::io(format!(
            "cannot create temporary file in {}: {error}",
            parent.display()
        ))
    })?;
    temp.write_all(data).map_err(|error| {
        SnipError::io(format!(
            "cannot write temporary file for {}: {error}",
            path.display()
        ))
    })?;
    temp.as_file().sync_all().map_err(|error| {
        SnipError::io(format!(
            "cannot sync temporary file for {}: {error}",
            path.display()
        ))
    })?;
    if let Some(permissions) = permissions {
        temp.as_file()
            .set_permissions(permissions)
            .map_err(|error| {
                SnipError::io(format!(
                    "cannot preserve permissions for {}: {error}",
                    path.display()
                ))
            })?;
    }
    temp.persist(path).map_err(|error| {
        SnipError::io(format!(
            "cannot replace {}: {}",
            path.display(),
            error.error
        ))
    })?;
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

pub fn write_snippet_manifest(path: &Path, manifest: &SnippetManifest) -> Result<()> {
    let data = toml::to_string_pretty(manifest)?;
    atomic_write(path, data.as_bytes())
}

pub(crate) fn validate_schema(version: u32, path: &Path) -> Result<()> {
    if version > SCHEMA_VERSION {
        return Err(SnipError::validation(format!(
            "{} uses schema version {version}, but this snip supports up to {SCHEMA_VERSION}",
            path.display()
        )));
    }
    if version == 0 {
        return Err(SnipError::validation(format!(
            "{} has invalid schema version 0",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn validate_snippet_manifest(manifest: &SnippetManifest, path: &Path) -> Result<()> {
    validate_schema(manifest.schema_version, path)?;
    if manifest.title.trim().is_empty() {
        return Err(SnipError::validation(format!(
            "{} has an empty title",
            path.display()
        )));
    }
    if manifest.fragments.is_empty() {
        return Err(SnipError::validation(format!(
            "{} must contain at least one fragment",
            path.display()
        )));
    }
    normalize_tags(&manifest.tags)?;
    for fragment in &manifest.fragments {
        validate_fragment(fragment, path)?;
    }
    Ok(())
}

fn validate_fragment(fragment: &FragmentManifest, manifest_path: &Path) -> Result<()> {
    if fragment.title.trim().is_empty() {
        return Err(SnipError::validation(format!(
            "{} has a fragment with an empty title",
            manifest_path.display()
        )));
    }
    if fragment.language.trim().is_empty() {
        return Err(SnipError::validation(format!(
            "{} has a fragment with an empty language",
            manifest_path.display()
        )));
    }
    let package = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    resolve_managed_path(package, &fragment.file)?;
    if let Some(note) = &fragment.note {
        resolve_managed_path(package, note)?;
    }
    Ok(())
}

pub(crate) fn read_safe_file(package: &Path, path: &Path) -> Result<Vec<u8>> {
    reject_symlink(path)?;
    let canonical_package = fs::canonicalize(package).map_err(|error| {
        SnipError::validation(format!("cannot resolve {}: {error}", package.display()))
    })?;
    let canonical_path = fs::canonicalize(path).map_err(|error| {
        SnipError::validation(format!("cannot resolve {}: {error}", path.display()))
    })?;
    if !canonical_path.starts_with(&canonical_package) {
        return Err(SnipError::validation(format!(
            "managed file escapes snippet package: {}",
            path.display()
        )));
    }
    fs::read(path)
        .map_err(|error| SnipError::validation(format!("cannot read {}: {error}", path.display())))
}

pub(crate) fn reject_symlink(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        SnipError::validation(format!("cannot inspect {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(SnipError::validation(format!(
            "symbolic links are not allowed in managed paths: {}",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn hash_entry(hasher: &mut blake3::Hasher, name: &str, data: &[u8]) {
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&(data.len() as u64).to_le_bytes());
    hasher.update(data);
}
