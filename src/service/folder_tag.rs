use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::snippet::edit_snippet;
use super::types::EditOptions;
use crate::error::{Result, SnipError};
use crate::filesystem::{Library, atomic_write};

pub fn create_folder(library: &Library, folder: &str) -> Result<PathBuf> {
    let _lock = library.lock()?;
    let relative = validate_folder(folder)?;
    if relative.as_os_str().is_empty() {
        return Err(SnipError::usage("folder path cannot be empty"));
    }
    let path = library.snippets_dir().join(relative);
    if path.exists() {
        return Err(SnipError::conflict(format!(
            "folder already exists: {}",
            path.display()
        )));
    }
    fs::create_dir_all(&path)?;
    atomic_write(&path.join(".keep"), b"")?;
    Ok(path)
}

pub fn move_folder(library: &Library, source: &str, target: &str) -> Result<PathBuf> {
    let _lock = library.lock()?;
    let source = library.snippets_dir().join(validate_folder(source)?);
    let target = library.snippets_dir().join(validate_folder(target)?);
    if source == library.snippets_dir() || target == library.snippets_dir() {
        return Err(SnipError::usage("cannot move the snippets root"));
    }
    if !source.is_dir() {
        return Err(SnipError::not_found(format!(
            "folder does not exist: {}",
            source.display()
        )));
    }
    if target.exists() {
        return Err(SnipError::conflict(format!(
            "target folder already exists: {}",
            target.display()
        )));
    }
    if target.starts_with(&source) {
        return Err(SnipError::usage("cannot move a folder inside itself"));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&source, &target)?;
    Ok(target)
}

pub fn delete_folder(library: &Library, folder: &str) -> Result<()> {
    let _lock = library.lock()?;
    let path = library.snippets_dir().join(validate_folder(folder)?);
    if path == library.snippets_dir() {
        return Err(SnipError::usage("cannot delete the snippets root"));
    }
    if !path.is_dir() {
        return Err(SnipError::not_found(format!(
            "folder does not exist: {}",
            path.display()
        )));
    }
    let entries = fs::read_dir(&path)?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_name() != ".keep")
        .collect::<Vec<_>>();
    if !entries.is_empty() {
        return Err(SnipError::conflict(format!(
            "folder is not empty: {}",
            path.display()
        )));
    }
    let keep = path.join(".keep");
    if keep.exists() {
        fs::remove_file(keep)?;
    }
    fs::remove_dir(path)?;
    Ok(())
}

pub fn rename_tag(library: &Library, old: &str, new: &str) -> Result<usize> {
    let replacement = new.trim();
    if replacement.is_empty() {
        return Err(SnipError::usage("new tag cannot be empty"));
    }
    let catalog = library.scan()?;
    let mut changed = 0;
    for snippet in catalog.snippets {
        if snippet.tags.iter().any(|tag| tag.eq_ignore_ascii_case(old)) {
            let tags = snippet
                .tags
                .iter()
                .map(|tag| {
                    if tag.eq_ignore_ascii_case(old) {
                        replacement.to_owned()
                    } else {
                        tag.clone()
                    }
                })
                .collect::<Vec<_>>();
            edit_snippet(
                library,
                &snippet.id.to_string(),
                &EditOptions {
                    tags: Some(tags),
                    if_hash: Some(snippet.fingerprint),
                    force: true,
                    ..EditOptions::default()
                },
            )?;
            changed += 1;
        }
    }
    let _lock = library.lock()?;
    let mut registry = library.tag_registry()?;
    if let Some(tag) = registry
        .tags
        .iter_mut()
        .find(|tag| tag.name.eq_ignore_ascii_case(old))
    {
        tag.name = replacement.to_owned();
    }
    // Editing snippets registers the replacement tag before the old registry
    // entry is renamed. Collapse that temporary duplicate atomically.
    let mut seen = HashSet::new();
    registry
        .tags
        .retain(|tag| seen.insert(tag.name.to_lowercase()));
    library.write_tag_registry(&registry)?;
    Ok(changed)
}

pub fn delete_tag(library: &Library, tag_to_delete: &str) -> Result<usize> {
    let catalog = library.scan()?;
    let mut changed = 0;
    for snippet in catalog.snippets {
        if snippet
            .tags
            .iter()
            .any(|tag| tag.eq_ignore_ascii_case(tag_to_delete))
        {
            let tags = snippet
                .tags
                .iter()
                .filter(|tag| !tag.eq_ignore_ascii_case(tag_to_delete))
                .cloned()
                .collect::<Vec<_>>();
            edit_snippet(
                library,
                &snippet.id.to_string(),
                &EditOptions {
                    tags: Some(tags),
                    if_hash: Some(snippet.fingerprint),
                    force: true,
                    ..EditOptions::default()
                },
            )?;
            changed += 1;
        }
    }
    let _lock = library.lock()?;
    let mut registry = library.tag_registry()?;
    let before = registry.tags.len();
    registry
        .tags
        .retain(|tag| !tag.name.eq_ignore_ascii_case(tag_to_delete));
    if registry.tags.len() != before {
        library.write_tag_registry(&registry)?;
    }
    Ok(changed)
}

pub(crate) fn validate_folder(folder: &str) -> Result<PathBuf> {
    let path = Path::new(folder.trim_matches('/'));
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(SnipError::validation(format!(
            "folder must be a relative path without . or ..: {folder:?}"
        )));
    }
    Ok(path.to_path_buf())
}
