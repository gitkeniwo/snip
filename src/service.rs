use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

use crate::domain::{
    ChangeSet, Fingerprint, FragmentManifest, SCHEMA_VERSION, Snippet, SnippetManifest,
    SourceMetadata,
};
use crate::error::{Result, SnipError};
use crate::filesystem::{
    Library, atomic_write, fragment_relative_path, normalize_tags, note_relative_path, now_rfc3339,
    package_name, resolve_managed_path, safe_component, write_snippet_manifest,
};

#[derive(Clone, Debug, Default)]
pub struct CreateOptions {
    pub id: Option<Uuid>,
    pub fragment_id: Option<Uuid>,
    pub title: String,
    pub folder: Option<String>,
    pub tags: Vec<String>,
    pub language: String,
    pub source_language: Option<String>,
    pub fragment_title: Option<String>,
    pub content: String,
    pub note: Option<String>,
    pub readme: Option<String>,
    pub pinned: bool,
    pub locked: bool,
    pub created_at: Option<String>,
    pub source: Option<SourceMetadata>,
}

#[derive(Clone, Debug, Default)]
pub struct EditOptions {
    pub title: Option<String>,
    pub folder: Option<String>,
    pub tags: Option<Vec<String>>,
    pub pinned: Option<bool>,
    pub locked: Option<bool>,
    pub fragment_selector: Option<String>,
    pub fragment_title: Option<String>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub note: Option<Option<String>>,
    pub readme: Option<Option<String>>,
    pub if_hash: Option<Fingerprint>,
    pub force: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FragmentAddOptions {
    pub id: Option<Uuid>,
    pub title: String,
    pub language: String,
    pub source_language: Option<String>,
    pub content: String,
    pub note: Option<String>,
    pub if_hash: Option<Fingerprint>,
    pub force: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FragmentEditOptions {
    pub title: Option<String>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub note: Option<Option<String>>,
    pub if_hash: Option<Fingerprint>,
    pub force: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DoctorReport {
    pub checked: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub pending_transactions: Vec<String>,
    pub repaired: Vec<String>,
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct TrashEntry {
    pub entry_id: String,
    pub deleted_at: String,
    pub original_path: String,
    pub snippet_id: Uuid,
    pub title: String,
    pub package_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrashMetadata {
    schema_version: u32,
    entry_id: String,
    deleted_at: String,
    original_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TransactionState {
    schema_version: u32,
    operation: String,
    original_path: String,
    target_path: String,
}

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
        library.write_tag_registry(&registry)?;
    }
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

pub fn organize(library: &Library, dry_run: bool) -> Result<Vec<ChangeSet>> {
    let _lock = library.lock()?;
    let catalog = library.scan()?;
    let mut changes = Vec::new();
    for snippet in catalog.snippets {
        let target = snippet
            .package_path
            .parent()
            .unwrap_or_else(|| library.root())
            .join(package_name(&snippet.title, snippet.id));
        if target == snippet.package_path {
            continue;
        }
        if target.exists() {
            return Err(SnipError::conflict(format!(
                "organize target already exists: {}",
                target.display()
            )));
        }
        if !dry_run {
            fs::rename(&snippet.package_path, &target)?;
        }
        changes.push(ChangeSet {
            fields: vec!["package_path".to_owned()],
            old_fingerprint: Some(snippet.fingerprint.clone()),
            new_fingerprint: Some(snippet.fingerprint),
            old_path: Some(snippet.package_path),
            new_path: Some(target),
        });
    }
    Ok(changes)
}

pub fn doctor(library: &Library, repair: bool) -> DoctorReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut repaired = Vec::new();
    let mut checked = 0;
    let pending_transactions = list_transaction_names(library);
    if let Err(error) = library.tag_registry() {
        errors.push(error.to_string());
    }
    if repair {
        for name in &pending_transactions {
            match recover_transaction(library, name) {
                Ok(message) => repaired.push(message),
                Err(error) => errors.push(format!("transaction {name}: {error}")),
            }
        }
    }
    match collect_package_paths(&library.snippets_dir()) {
        Ok(paths) => {
            let mut ids = HashSet::new();
            for path in paths {
                checked += 1;
                match library.load_snippet(&path) {
                    Ok(snippet) => {
                        if !ids.insert(snippet.id) {
                            errors.push(format!("duplicate snippet UUID: {}", snippet.id));
                        }
                        let expected = package_name(&snippet.title, snippet.id);
                        if path.file_name().and_then(|value| value.to_str()) != Some(&expected) {
                            warnings.push(format!(
                                "{}: package name differs from canonical {expected:?}",
                                path.display()
                            ));
                        }
                    }
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
            }
        }
        Err(error) => errors.push(error.to_string()),
    }
    let active_pending = if repair {
        list_transaction_names(library)
    } else {
        pending_transactions
    };
    let ok = errors.is_empty() && active_pending.is_empty();
    DoctorReport {
        checked,
        errors,
        warnings,
        pending_transactions: active_pending,
        repaired,
        ok,
    }
}

fn replace_package<F>(
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

fn ensure_hash(snippet: &Snippet, expected: Option<&Fingerprint>) -> Result<()> {
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

fn ensure_mutable(snippet: &Snippet, force: bool) -> Result<()> {
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

fn resolve_fragment_index(manifest: &SnippetManifest, selector: Option<&str>) -> Result<usize> {
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

fn validate_folder(folder: &str) -> Result<PathBuf> {
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

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(SnipError::validation(format!(
            "symbolic links are not allowed: {}",
            source.display()
        )));
    }
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination = target.join(entry.file_name());
        if file_type.is_symlink() {
            return Err(SnipError::validation(format!(
                "symbolic links are not allowed: {}",
                entry.path().display()
            )));
        }
        if file_type.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

fn collect_package_paths(root: &Path) -> Result<Vec<PathBuf>> {
    fn walk(path: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Err(SnipError::validation(format!(
                "symbolic links are not allowed: {}",
                path.display()
            )));
        }
        if path.join("snippet.toml").is_file() {
            result.push(path.to_path_buf());
            return Ok(());
        }
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), result)?;
            }
        }
        Ok(())
    }
    let mut result = Vec::new();
    walk(root, &mut result)?;
    result.sort();
    Ok(result)
}

fn list_transaction_names(library: &Library) -> Vec<String> {
    fs::read_dir(library.transactions_dir())
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn recover_transaction(library: &Library, name: &str) -> Result<String> {
    let directory = library.transactions_dir().join(name);
    let state_path = directory.join("transaction.toml");
    if !state_path.is_file() {
        return Err(SnipError::validation("missing transaction.toml"));
    }
    let state: TransactionState = toml::from_str(&fs::read_to_string(&state_path)?)?;
    let original = library.root().join(&state.original_path);
    let target = library.root().join(&state.target_path);
    let backup = directory.join("backup");
    let staged = directory.join("staged");
    if target.exists() {
        if backup.exists() {
            fs::remove_dir_all(&backup)?;
        }
        if staged.exists() {
            fs::remove_dir_all(&staged)?;
        }
        fs::remove_dir_all(&directory)?;
        return Ok(format!("completed transaction {name}"));
    }
    if backup.exists() {
        if let Some(parent) = original.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&backup, &original)?;
    }
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    fs::remove_dir_all(&directory)?;
    Ok(format!("rolled back transaction {name}"))
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

fn relative_to_root(library: &Library, path: &Path) -> Result<String> {
    path.strip_prefix(library.root())
        .map(path_to_slashes)
        .map_err(|_| SnipError::validation(format!("{} is outside library", path.display())))
}

fn path_to_slashes(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn remove_keep_file(path: &Path) {
    let keep = path.join(".keep");
    if keep.exists() {
        let _ = fs::remove_file(keep);
    }
}

fn ensure_keep_for_empty_parents(library: &Library, start: Option<&Path>) {
    let Some(path) = start else {
        return;
    };
    if path == library.snippets_dir() || !path.starts_with(library.snippets_dir()) {
        return;
    }
    if fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none()) {
        let _ = atomic_write(&path.join(".keep"), b"");
    }
}
