use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::folder_tag::validate_folder;
use super::fragment::resolve_fragment_index;
use super::helpers::{
    copy_tree, ensure_keep_for_empty_parents, relative_to_root, remove_keep_file,
};
use super::types::{CreateOptions, EditOptions, TransactionState};
use crate::domain::{
    ChangeSet, Fingerprint, FragmentManifest, SCHEMA_VERSION, Snippet, SnippetManifest,
};
use crate::error::{Result, SnipError};
use crate::filesystem::{
    Library, atomic_write, fragment_relative_path, normalize_tags, note_relative_path, now_rfc3339,
    package_name, resolve_managed_path, write_snippet_manifest,
};

pub fn create_snippet(library: &Library, options: &CreateOptions) -> Result<Snippet> {
    let _lock = library.lock()?;
    create_snippet_unlocked(library, options)
}

pub(crate) fn create_snippet_unlocked(
    library: &Library,
    options: &CreateOptions,
) -> Result<Snippet> {
    let title = options.title.trim();
    if title.is_empty() {
        return Err(SnipError::usage("snippet title cannot be empty"));
    }
    let language = if options.language.trim().is_empty() {
        "text"
    } else {
        options.language.trim()
    };
    let folder = validate_folder(options.folder.as_deref().unwrap_or(""))?;
    let parent = if folder.as_os_str().is_empty() {
        library.snippets_dir()
    } else {
        library.snippets_dir().join(&folder)
    };
    fs::create_dir_all(&parent).map_err(|error| {
        SnipError::io(format!(
            "cannot create folder {}: {error}",
            parent.display()
        ))
    })?;
    remove_keep_file(&parent);

    let id = options.id.unwrap_or_else(Uuid::new_v4);
    let final_path = parent.join(package_name(title, id));
    if final_path.exists() {
        return Err(SnipError::conflict(format!(
            "snippet package already exists: {}",
            final_path.display()
        )));
    }
    let stage = parent.join(format!(".snip-stage-{}", Uuid::new_v4().simple()));
    fs::create_dir_all(stage.join("fragments"))?;
    fs::create_dir_all(stage.join("notes"))?;
    fs::create_dir_all(stage.join("attachments"))?;

    let fragment_id = options.fragment_id.unwrap_or_else(Uuid::new_v4);
    let fragment_title = options.fragment_title.as_deref().unwrap_or("Fragment");
    let fragment_file = fragment_relative_path(1, title, language);
    atomic_write(
        &resolve_managed_path(&stage, &fragment_file)?,
        options.content.as_bytes(),
    )?;
    let note_path = if let Some(note) = &options.note {
        let path = note_relative_path(1);
        atomic_write(&resolve_managed_path(&stage, &path)?, note.as_bytes())?;
        Some(path)
    } else {
        None
    };
    if let Some(readme) = &options.readme {
        atomic_write(&stage.join("README.md"), readme.as_bytes())?;
    }
    let manifest = SnippetManifest {
        schema_version: SCHEMA_VERSION,
        id,
        title: title.to_owned(),
        tags: normalize_tags(&options.tags)?,
        pinned: options.pinned,
        locked: options.locked,
        created_at: options.created_at.clone().unwrap_or(now_rfc3339()?),
        source: options.source.clone(),
        fragments: vec![FragmentManifest {
            id: fragment_id,
            title: fragment_title.to_owned(),
            language: language.to_owned(),
            file: fragment_file,
            note: note_path,
            source_language: options.source_language.clone(),
            extra: toml::Table::new(),
        }],
        extra: toml::Table::new(),
    };
    write_snippet_manifest(&stage.join("snippet.toml"), &manifest)?;
    if let Err(error) = library.load_snippet(&stage) {
        let _ = fs::remove_dir_all(&stage);
        return Err(error);
    }
    fs::rename(&stage, &final_path).map_err(|error| {
        let _ = fs::remove_dir_all(&stage);
        SnipError::io(format!(
            "cannot commit snippet {}: {error}",
            final_path.display()
        ))
    })?;
    library.register_tags(&manifest.tags)?;
    library.load_snippet(&final_path)
}

pub fn edit_snippet(
    library: &Library,
    selector: &str,
    options: &EditOptions,
) -> Result<(Snippet, ChangeSet)> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    ensure_mutable(&snippet, options.force)?;
    ensure_hash(&snippet, options.if_hash.as_ref())?;
    if !has_edit_changes(options) {
        return Err(SnipError::usage("no changes requested"));
    }
    let mut target_folder = PathBuf::from(&snippet.folder);
    if let Some(folder) = &options.folder {
        target_folder = validate_folder(folder)?;
    }
    let target_parent = if target_folder.as_os_str().is_empty() {
        library.snippets_dir()
    } else {
        library.snippets_dir().join(&target_folder)
    };
    fs::create_dir_all(&target_parent)?;
    remove_keep_file(&target_parent);
    let target_title = options.title.as_deref().unwrap_or(&snippet.title);
    let target_path = if options.title.is_some() || options.folder.is_some() {
        target_parent.join(package_name(target_title, snippet.id))
    } else {
        snippet.package_path.clone()
    };
    let old_fingerprint = snippet.fingerprint.clone();
    let old_path = snippet.package_path.clone();
    let mut fields = Vec::new();

    let result = replace_package(library, &snippet, &target_path, |stage, manifest| {
        if let Some(title) = &options.title {
            if title.trim().is_empty() {
                return Err(SnipError::usage("snippet title cannot be empty"));
            }
            manifest.title = title.trim().to_owned();
            fields.push("title".to_owned());
        }
        if options.folder.is_some() {
            fields.push("folder".to_owned());
        }
        if let Some(tags) = &options.tags {
            manifest.tags = normalize_tags(tags)?;
            fields.push("tags".to_owned());
        }
        if let Some(pinned) = options.pinned {
            manifest.pinned = pinned;
            fields.push("pinned".to_owned());
        }
        if let Some(locked) = options.locked {
            manifest.locked = locked;
            fields.push("locked".to_owned());
        }
        if let Some(readme) = &options.readme {
            let path = stage.join("README.md");
            match readme {
                Some(value) => atomic_write(&path, value.as_bytes())?,
                None if path.exists() => fs::remove_file(&path)?,
                None => {}
            }
            fields.push("readme".to_owned());
        }
        if options.fragment_title.is_some()
            || options.language.is_some()
            || options.content.is_some()
            || options.note.is_some()
        {
            let index = resolve_fragment_index(manifest, options.fragment_selector.as_deref())?;
            let fragment = &mut manifest.fragments[index];
            if let Some(title) = &options.fragment_title {
                if title.trim().is_empty() {
                    return Err(SnipError::usage("fragment title cannot be empty"));
                }
                fragment.title = title.trim().to_owned();
                fields.push(format!("fragments[{}].title", index + 1));
            }
            if let Some(language) = &options.language {
                if language.trim().is_empty() {
                    return Err(SnipError::usage("fragment language cannot be empty"));
                }
                fragment.language = language.trim().to_owned();
                fields.push(format!("fragments[{}].language", index + 1));
            }
            if let Some(content) = &options.content {
                atomic_write(
                    &resolve_managed_path(stage, &fragment.file)?,
                    content.as_bytes(),
                )?;
                fields.push(format!("fragments[{}].content", index + 1));
            }
            if let Some(note) = &options.note {
                match note {
                    Some(value) => {
                        let relative = fragment
                            .note
                            .clone()
                            .unwrap_or_else(|| note_relative_path(index + 1));
                        atomic_write(&resolve_managed_path(stage, &relative)?, value.as_bytes())?;
                        fragment.note = Some(relative);
                    }
                    None => {
                        if let Some(relative) = fragment.note.take() {
                            let path = resolve_managed_path(stage, &relative)?;
                            if path.exists() {
                                fs::remove_file(path)?;
                            }
                        }
                    }
                }
                fields.push(format!("fragments[{}].note", index + 1));
            }
        }
        Ok(())
    })?;
    library.register_tags(&result.tags)?;
    Ok((
        result.clone(),
        ChangeSet {
            fields,
            old_fingerprint: Some(old_fingerprint),
            new_fingerprint: Some(result.fingerprint.clone()),
            old_path: Some(old_path),
            new_path: Some(result.package_path.clone()),
        },
    ))
}

pub fn replace_manifest_text(
    library: &Library,
    selector: &str,
    manifest_text: &str,
    if_hash: Option<&Fingerprint>,
    force: bool,
) -> Result<(Snippet, ChangeSet)> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?.clone();
    ensure_mutable(&snippet, force)?;
    ensure_hash(&snippet, if_hash)?;
    let replacement: SnippetManifest = toml::from_str(manifest_text)?;
    if replacement.id != snippet.id {
        return Err(SnipError::conflict(
            "editing snippet.toml cannot change the snippet UUID",
        ));
    }
    if replacement.fragments.len() != snippet.fragments.len()
        || replacement.fragments.iter().any(|fragment| {
            !snippet
                .fragments
                .iter()
                .any(|old| old.id == fragment.id && old.file == fragment.file)
        })
    {
        return Err(SnipError::conflict(
            "metadata editor cannot add fragments or change fragment file paths; use snip fragment",
        ));
    }
    let target_folder = PathBuf::from(&snippet.folder);
    let target_parent = if target_folder.as_os_str().is_empty() {
        library.snippets_dir()
    } else {
        library.snippets_dir().join(target_folder)
    };
    let target_path = if replacement.title != snippet.title {
        target_parent.join(package_name(&replacement.title, replacement.id))
    } else {
        snippet.package_path.clone()
    };
    let old_fingerprint = snippet.fingerprint.clone();
    let result = replace_package(library, &snippet, &target_path, |_stage, manifest| {
        *manifest = replacement;
        Ok(())
    })?;
    library.register_tags(&result.tags)?;
    Ok((
        result.clone(),
        ChangeSet {
            fields: vec!["manifest".to_owned()],
            old_fingerprint: Some(old_fingerprint),
            new_fingerprint: Some(result.fingerprint.clone()),
            old_path: Some(snippet.package_path.clone()),
            new_path: Some(result.package_path.clone()),
        },
    ))
}

pub(crate) fn replace_package<F>(
    library: &Library,
    snippet: &Snippet,
    target_path: &Path,
    mutate: F,
) -> Result<Snippet>
where
    F: FnOnce(&Path, &mut SnippetManifest) -> Result<()>,
{
    if target_path != snippet.package_path && target_path.exists() {
        return Err(SnipError::conflict(format!(
            "target package already exists: {}",
            target_path.display()
        )));
    }
    let transaction_id = Uuid::new_v4().simple().to_string();
    let transaction_dir = library.transactions_dir().join(&transaction_id);
    let stage = transaction_dir.join("staged");
    let backup = transaction_dir.join("backup");
    fs::create_dir_all(&transaction_dir)?;
    copy_tree(&snippet.package_path, &stage)?;
    let manifest_path = stage.join("snippet.toml");
    let mut manifest: SnippetManifest = toml::from_str(&fs::read_to_string(&manifest_path)?)?;
    if let Err(error) = mutate(&stage, &mut manifest) {
        let _ = fs::remove_dir_all(&transaction_dir);
        return Err(error);
    }
    write_snippet_manifest(&manifest_path, &manifest)?;
    if let Err(error) = library.load_snippet(&stage) {
        let _ = fs::remove_dir_all(&transaction_dir);
        return Err(error);
    }
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let state = TransactionState {
        schema_version: SCHEMA_VERSION,
        operation: "replace".to_owned(),
        original_path: relative_to_root(library, &snippet.package_path)?,
        target_path: relative_to_root(library, target_path)?,
    };
    atomic_write(
        &transaction_dir.join("transaction.toml"),
        toml::to_string_pretty(&state)?.as_bytes(),
    )?;
    fs::rename(&snippet.package_path, &backup)?;
    if let Err(error) = fs::rename(&stage, target_path) {
        let _ = fs::rename(&backup, &snippet.package_path);
        return Err(SnipError::io(format!("cannot commit transaction: {error}")));
    }
    let loaded = match library.load_snippet(target_path) {
        Ok(value) => value,
        Err(error) => {
            let _ = fs::remove_dir_all(target_path);
            let _ = fs::rename(&backup, &snippet.package_path);
            return Err(error);
        }
    };
    let _ = fs::remove_dir_all(&backup);
    let _ = fs::remove_dir_all(&transaction_dir);
    if snippet.package_path != target_path {
        ensure_keep_for_empty_parents(library, snippet.package_path.parent());
    }
    Ok(loaded)
}

pub(crate) fn ensure_hash(snippet: &Snippet, expected: Option<&Fingerprint>) -> Result<()> {
    if let Some(expected) = expected
        && expected != &snippet.fingerprint
    {
        return Err(SnipError::conflict(format!(
            "snippet changed since it was read: expected {}, found {}",
            expected, snippet.fingerprint
        )));
    }
    Ok(())
}

pub(crate) fn ensure_mutable(snippet: &Snippet, force: bool) -> Result<()> {
    if snippet.locked && !force {
        return Err(SnipError::conflict(format!(
            "snippet {} is locked; pass --force to modify it",
            snippet.id
        )));
    }
    Ok(())
}

fn has_edit_changes(options: &EditOptions) -> bool {
    options.title.is_some()
        || options.folder.is_some()
        || options.tags.is_some()
        || options.pinned.is_some()
        || options.locked.is_some()
        || options.fragment_title.is_some()
        || options.language.is_some()
        || options.content.is_some()
        || options.note.is_some()
        || options.readme.is_some()
}
