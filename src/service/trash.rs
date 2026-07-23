use std::fs;
use uuid::Uuid;

use super::folder_tag::validate_folder;
use super::helpers::{ensure_keep_for_empty_parents, path_to_slashes, remove_keep_file};
use super::snippet::{ensure_hash, ensure_mutable};
use super::types::{TrashEntry, TrashMetadata};
use crate::domain::{Fingerprint, SCHEMA_VERSION, Snippet};
use crate::error::{Result, SnipError};
use crate::filesystem::{Library, atomic_write, now_rfc3339, package_name, safe_component};

pub fn delete_snippet(
    library: &Library,
    selector: &str,
    if_hash: Option<&Fingerprint>,
    force: bool,
) -> Result<TrashEntry> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    ensure_mutable(&snippet, force)?;
    ensure_hash(&snippet, if_hash)?;
    let entry_id = Uuid::new_v4().simple().to_string();
    let deleted_at = now_rfc3339()?;
    let stamp = deleted_at
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .take(14)
        .collect::<String>();
    let entry_dir = library.trash_dir().join(format!(
        "{stamp}-{}-{entry_id}",
        safe_component(&snippet.title)
    ));
    fs::create_dir_all(&entry_dir)?;
    let original_path = snippet
        .package_path
        .strip_prefix(library.root())
        .map(path_to_slashes)
        .map_err(|_| SnipError::validation("snippet path is outside library"))?;
    let metadata = TrashMetadata {
        schema_version: SCHEMA_VERSION,
        entry_id: entry_id.clone(),
        deleted_at: deleted_at.clone(),
        original_path: original_path.clone(),
    };
    atomic_write(
        &entry_dir.join("trash.toml"),
        toml::to_string_pretty(&metadata)?.as_bytes(),
    )?;
    let package_path = entry_dir.join("package");
    if let Err(error) = fs::rename(&snippet.package_path, &package_path) {
        let _ = fs::remove_dir_all(&entry_dir);
        return Err(SnipError::io(format!(
            "cannot move snippet to trash: {error}"
        )));
    }
    ensure_keep_for_empty_parents(library, snippet.package_path.parent());
    Ok(TrashEntry {
        entry_id,
        deleted_at,
        original_path,
        snippet_id: snippet.id,
        title: snippet.title.clone(),
        package_path,
    })
}

pub fn trash_entries(library: &Library) -> Result<Vec<TrashEntry>> {
    let mut result = Vec::new();
    let mut entries =
        fs::read_dir(library.trash_dir())?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let metadata_path = entry.path().join("trash.toml");
        let package_path = entry.path().join("package");
        if !metadata_path.is_file() || !package_path.is_dir() {
            continue;
        }
        let metadata: TrashMetadata = toml::from_str(&fs::read_to_string(&metadata_path)?)?;
        let snippet = library.load_snippet(&package_path)?;
        result.push(TrashEntry {
            entry_id: metadata.entry_id,
            deleted_at: metadata.deleted_at,
            original_path: metadata.original_path,
            snippet_id: snippet.id,
            title: snippet.title.clone(),
            package_path,
        });
    }
    Ok(result)
}

pub fn restore_snippet(
    library: &Library,
    selector: &str,
    target_folder: Option<&str>,
) -> Result<Snippet> {
    let _lock = library.lock()?;
    let entries = trash_entries(library)?;
    let entry = resolve_trash_entry(&entries, selector)?.clone();
    let target = if let Some(folder) = target_folder {
        let parent = library.snippets_dir().join(validate_folder(folder)?);
        fs::create_dir_all(&parent)?;
        let snippet = library.load_snippet(&entry.package_path)?;
        parent.join(package_name(&snippet.title, snippet.id))
    } else {
        library.root().join(&entry.original_path)
    };
    if target.exists() {
        return Err(SnipError::conflict(format!(
            "restore target already exists: {}; pass --folder",
            target.display()
        )));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
        remove_keep_file(parent);
    }
    fs::rename(&entry.package_path, &target)?;
    if let Some(wrapper) = entry.package_path.parent() {
        let _ = fs::remove_file(wrapper.join("trash.toml"));
        let _ = fs::remove_dir(wrapper);
    }
    library.load_snippet(&target)
}

pub fn purge_snippet(library: &Library, selector: &str) -> Result<TrashEntry> {
    let _lock = library.lock()?;
    let entries = trash_entries(library)?;
    let entry = resolve_trash_entry(&entries, selector)?.clone();
    let wrapper = entry
        .package_path
        .parent()
        .ok_or_else(|| SnipError::validation("trash entry has no wrapper directory"))?;
    fs::remove_dir_all(wrapper)?;
    Ok(entry)
}

fn resolve_trash_entry<'a>(entries: &'a [TrashEntry], selector: &str) -> Result<&'a TrashEntry> {
    let lower = selector.to_lowercase();
    let matches = entries
        .iter()
        .filter(|entry| {
            entry.entry_id.starts_with(&lower)
                || entry
                    .snippet_id
                    .simple()
                    .to_string()
                    .starts_with(&lower.replace('-', ""))
                || entry.title == selector
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(SnipError::not_found(format!(
            "no trash entry matches {selector:?}"
        ))),
        [entry] => Ok(*entry),
        _ => Err(SnipError::not_found(format!(
            "ambiguous trash selector {selector:?}: {} matches",
            matches.len()
        ))),
    }
}
