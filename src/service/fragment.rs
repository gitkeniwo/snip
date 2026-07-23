use std::fs;
use uuid::Uuid;

use super::snippet::{edit_snippet, ensure_hash, ensure_mutable, replace_package};
use super::types::{EditOptions, FragmentAddOptions, FragmentEditOptions};
use crate::domain::{ChangeSet, Fingerprint, FragmentManifest, Snippet, SnippetManifest};
use crate::error::{Result, SnipError};
use crate::filesystem::{
    Library, atomic_write, fragment_relative_path, note_relative_path, resolve_managed_path,
};

pub fn add_fragment(
    library: &Library,
    selector: &str,
    options: &FragmentAddOptions,
) -> Result<(Snippet, ChangeSet)> {
    if options.title.trim().is_empty() || options.language.trim().is_empty() {
        return Err(SnipError::usage(
            "fragment title and language cannot be empty",
        ));
    }
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    ensure_mutable(&snippet, options.force)?;
    ensure_hash(&snippet, options.if_hash.as_ref())?;
    let old_fingerprint = snippet.fingerprint.clone();
    let result = replace_package(
        library,
        &snippet,
        &snippet.package_path,
        |stage, manifest| {
            let index = manifest.fragments.len() + 1;
            let file = fragment_relative_path(index, &options.title, &options.language);
            atomic_write(
                &resolve_managed_path(stage, &file)?,
                options.content.as_bytes(),
            )?;
            let note = if let Some(value) = &options.note {
                let relative = note_relative_path(index);
                atomic_write(&resolve_managed_path(stage, &relative)?, value.as_bytes())?;
                Some(relative)
            } else {
                None
            };
            manifest.fragments.push(FragmentManifest {
                id: options.id.unwrap_or_else(Uuid::new_v4),
                title: options.title.trim().to_owned(),
                language: options.language.trim().to_owned(),
                file,
                note,
                source_language: options.source_language.clone(),
                extra: toml::Table::new(),
            });
            Ok(())
        },
    )?;
    library.register_tags(&result.tags)?;
    Ok((
        result.clone(),
        ChangeSet {
            fields: vec!["fragments.add".to_owned()],
            old_fingerprint: Some(old_fingerprint),
            new_fingerprint: Some(result.fingerprint.clone()),
            old_path: Some(snippet.package_path.clone()),
            new_path: Some(result.package_path.clone()),
        },
    ))
}

pub fn edit_fragment(
    library: &Library,
    selector: &str,
    fragment_selector: &str,
    options: &FragmentEditOptions,
) -> Result<(Snippet, ChangeSet)> {
    let edit = EditOptions {
        fragment_selector: Some(fragment_selector.to_owned()),
        fragment_title: options.title.clone(),
        language: options.language.clone(),
        content: options.content.clone(),
        note: options.note.clone(),
        if_hash: options.if_hash.clone(),
        force: options.force,
        ..EditOptions::default()
    };
    edit_snippet(library, selector, &edit)
}

pub fn remove_fragment(
    library: &Library,
    selector: &str,
    fragment_selector: &str,
    if_hash: Option<&Fingerprint>,
    force: bool,
) -> Result<(Snippet, ChangeSet)> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    ensure_mutable(&snippet, force)?;
    ensure_hash(&snippet, if_hash)?;
    if snippet.fragments.len() == 1 {
        return Err(SnipError::conflict(
            "cannot remove the only fragment; delete the snippet instead",
        ));
    }
    let old_fingerprint = snippet.fingerprint.clone();
    let result = replace_package(
        library,
        &snippet,
        &snippet.package_path,
        |stage, manifest| {
            let index = resolve_fragment_index(manifest, Some(fragment_selector))?;
            let fragment = manifest.fragments.remove(index);
            let content = resolve_managed_path(stage, &fragment.file)?;
            if content.exists() {
                fs::remove_file(content)?;
            }
            if let Some(note) = fragment.note {
                let note = resolve_managed_path(stage, &note)?;
                if note.exists() {
                    fs::remove_file(note)?;
                }
            }
            Ok(())
        },
    )?;
    Ok((
        result.clone(),
        ChangeSet {
            fields: vec!["fragments.remove".to_owned()],
            old_fingerprint: Some(old_fingerprint),
            new_fingerprint: Some(result.fingerprint.clone()),
            old_path: Some(snippet.package_path.clone()),
            new_path: Some(result.package_path.clone()),
        },
    ))
}

pub fn reorder_fragment(
    library: &Library,
    selector: &str,
    fragment_selector: &str,
    position: usize,
    if_hash: Option<&Fingerprint>,
    force: bool,
) -> Result<(Snippet, ChangeSet)> {
    if position == 0 {
        return Err(SnipError::usage("fragment positions start at 1"));
    }
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    ensure_mutable(&snippet, force)?;
    ensure_hash(&snippet, if_hash)?;
    let old_fingerprint = snippet.fingerprint.clone();
    let result = replace_package(
        library,
        &snippet,
        &snippet.package_path,
        |_stage, manifest| {
            if position > manifest.fragments.len() {
                return Err(SnipError::not_found(format!(
                    "position {position} is out of range; snippet has {} fragments",
                    manifest.fragments.len()
                )));
            }
            let index = resolve_fragment_index(manifest, Some(fragment_selector))?;
            let fragment = manifest.fragments.remove(index);
            manifest.fragments.insert(position - 1, fragment);
            Ok(())
        },
    )?;
    Ok((
        result.clone(),
        ChangeSet {
            fields: vec!["fragments.reorder".to_owned()],
            old_fingerprint: Some(old_fingerprint),
            new_fingerprint: Some(result.fingerprint.clone()),
            old_path: Some(snippet.package_path.clone()),
            new_path: Some(result.package_path.clone()),
        },
    ))
}

pub(crate) fn resolve_fragment_index(
    manifest: &SnippetManifest,
    selector: Option<&str>,
) -> Result<usize> {
    let Some(selector) = selector else {
        return Ok(0);
    };
    if let Ok(index) = selector.parse::<usize>() {
        if index == 0 {
            return Err(SnipError::usage("fragment indices start at 1"));
        }
        return (index <= manifest.fragments.len())
            .then_some(index - 1)
            .ok_or_else(|| {
                SnipError::not_found(format!(
                    "fragment index {index} is out of range; snippet has {} fragments",
                    manifest.fragments.len()
                ))
            });
    }
    let compact = selector.replace('-', "").to_lowercase();
    if compact.len() < 8 || !compact.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(SnipError::usage(
            "fragment selector must be a 1-based index or UUID prefix of at least 8 hex digits",
        ));
    }
    let matches = manifest
        .fragments
        .iter()
        .enumerate()
        .filter(|(_, fragment)| fragment.id.simple().to_string().starts_with(&compact))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(SnipError::not_found(format!(
            "no fragment UUID matches {selector:?}"
        ))),
        [index] => Ok(*index),
        _ => Err(SnipError::not_found(format!(
            "ambiguous fragment UUID {selector:?}: {} matches",
            matches.len()
        ))),
    }
}
