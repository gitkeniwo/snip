use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

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
    validate_snippet_manifest(manifest, path)?;
    validate_snippet_metadata(manifest, path)?;
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

/// Semantic checks added after schema v1 libraries already existed.
///
/// Reads stay tolerant so one hand-edited metadata value does not hide an
/// otherwise usable library. Writers call this before persisting a manifest,
/// and `doctor` reports violations in existing libraries.
pub(crate) fn validate_snippet_metadata(manifest: &SnippetManifest, path: &Path) -> Result<()> {
    validate_rfc3339(&manifest.created_at, "created_at", path)?;
    if let Some(source) = &manifest.source {
        if source.kind.trim().is_empty() {
            return Err(SnipError::validation(format!(
                "{} has source metadata with an empty kind",
                path.display()
            )));
        }
        if let Some(modified_at) = &source.modified_at {
            validate_rfc3339(modified_at, "source.modified_at", path)?;
        }
    }
    let mut remote_kinds = std::collections::HashSet::new();
    for remote in &manifest.remotes {
        let kind = remote.kind.trim();
        if kind.is_empty() {
            return Err(SnipError::validation(format!(
                "{} has a remote with an empty kind",
                path.display()
            )));
        }
        if !remote_kinds.insert(kind) {
            return Err(SnipError::validation(format!(
                "{} has duplicate remote kind {kind:?}",
                path.display()
            )));
        }
        for (field, value) in [
            ("host", remote.host.as_str()),
            ("id", remote.id.as_str()),
            ("url", remote.url.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(SnipError::validation(format!(
                    "{} has a remote with an empty {field}",
                    path.display()
                )));
            }
        }
        if let Some(pushed_at) = &remote.pushed_at {
            validate_rfc3339(pushed_at, "remotes.pushed_at", path)?;
        }
        let mut filenames = std::collections::HashSet::new();
        for filename in &remote.files {
            if filename.trim().is_empty() || !filenames.insert(filename) {
                return Err(SnipError::validation(format!(
                    "{} has an empty or duplicate remote filename",
                    path.display()
                )));
            }
        }
        if let Some(digest) = &remote.pushed_digest
            && (digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
        {
            return Err(SnipError::validation(format!(
                "{} has an invalid remote pushed_digest",
                path.display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_rfc3339(value: &str, field: &str, path: &Path) -> Result<()> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|error| {
        SnipError::validation(format!(
            "{} has invalid RFC 3339 {field} {value:?}: {error}",
            path.display()
        ))
    })?;
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

pub(crate) fn reject_symlinks_recursive(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        SnipError::validation(format!("cannot inspect {}: {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(SnipError::validation(format!(
            "symbolic links are not allowed in managed paths: {}",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    reject_symlinks_below(path)
}

fn reject_symlinks_below(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path).map_err(|error| {
        SnipError::validation(format!("cannot enumerate {}: {error}", path.display()))
    })? {
        let entry = entry.map_err(|error| {
            SnipError::validation(format!("cannot enumerate {}: {error}", path.display()))
        })?;
        let file_type = entry.file_type().map_err(|error| {
            SnipError::validation(format!(
                "cannot inspect {}: {error}",
                entry.path().display()
            ))
        })?;
        if file_type.is_symlink() {
            return Err(SnipError::validation(format!(
                "symbolic links are not allowed in managed paths: {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            reject_symlinks_below(&entry.path())?;
        }
    }
    Ok(())
}

pub(crate) fn hash_entry(hasher: &mut blake3::Hasher, name: &str, data: &[u8]) {
    hasher.update(&(name.len() as u64).to_le_bytes());
    hasher.update(name.as_bytes());
    hasher.update(&(data.len() as u64).to_le_bytes());
    hasher.update(data);
}
