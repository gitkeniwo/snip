use serde_json::json;
use snip::Library;
use snip::config::AppConfig;
use snip::domain::{CatalogSnapshot, Fingerprint, Snippet};
use snip::error::{ErrorKind, Result, SnipError};
use snip::external_editor::{
    EditorTargetKind, editor_dir_for_target, launch_editor, resolve_editor_cwd,
};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, FragmentEditOptions, add_fragment,
    create_snippet, delete_snippet, edit_fragment, edit_snippet, remove_fragment, reorder_fragment,
    replace_manifest_text,
};
use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::Builder;

use super::output::{print_mutation, print_record};
use crate::cli::{CreateArgs, DeleteArgs, EditArgs, FragmentArgs, FragmentCommand, OutputMode};

pub fn command_create(
    library: &Library,
    args: &CreateArgs,
    output: OutputMode,
    config: &AppConfig,
) -> Result<()> {
    let tags = if args.tags.is_empty() {
        config.default_tags.clone()
    } else {
        args.tags.clone()
    };
    let snippet = create_snippet(
        library,
        &CreateOptions {
            title: args.title.clone(),
            folder: args
                .folder
                .clone()
                .or_else(|| config.default_folder.clone()),
            tags,
            language: args
                .language
                .clone()
                .or_else(|| config.default_language.clone())
                .unwrap_or_else(|| "text".to_owned()),
            source_language: None,
            fragment_title: Some(args.fragment_title.clone()),
            content: read_optional_text(args.content.as_deref(), args.content_file.as_deref())?
                .unwrap_or_default(),
            note: read_optional_text(args.note.as_deref(), args.note_file.as_deref())?,
            readme: read_optional_text(args.readme.as_deref(), args.readme_file.as_deref())?,
            pinned: args.pin,
            locked: args.lock,
            ..CreateOptions::default()
        },
    )?;
    print_mutation(&snippet, None, output)
}

pub fn command_edit(
    library: &Library,
    args: &EditArgs,
    output: OutputMode,
    config: &AppConfig,
) -> Result<()> {
    if args.create && edit_has_create_incompatible_changes(args) {
        return Err(SnipError::usage(
            "--create cannot be combined with structured changes; use snip create instead",
        ));
    }
    if args.create && args.optimistic.if_hash.is_some() {
        return Err(SnipError::usage(
            "--create cannot be combined with --if-hash",
        ));
    }
    if args.create || !edit_has_structured_changes(args) {
        return edit_external(library, args, output, config);
    }
    let options = EditOptions {
        title: args.title.clone(),
        folder: args.folder.clone(),
        tags: if args.clear_tags {
            Some(Vec::new())
        } else if !args.tags.is_empty() {
            Some(args.tags.clone())
        } else {
            None
        },
        pinned: args.pin.then_some(true).or(args.unpin.then_some(false)),
        locked: args.lock.then_some(true).or(args.unlock.then_some(false)),
        fragment_selector: args.fragment.clone(),
        fragment_title: args.fragment_title.clone(),
        language: args.language.clone(),
        content: read_optional_text(args.content.as_deref(), args.content_file.as_deref())?,
        note: if args.clear_note {
            Some(None)
        } else {
            read_optional_text(args.note.as_deref(), args.note_file.as_deref())?.map(Some)
        },
        readme: if args.clear_readme {
            Some(None)
        } else {
            read_optional_text(args.readme.as_deref(), args.readme_file.as_deref())?.map(Some)
        },
        if_hash: fingerprint(args.optimistic.if_hash.as_deref()),
        force: args.optimistic.force,
    };
    let (snippet, changes) = edit_snippet(library, &args.selector, &options)?;
    print_mutation(&snippet, Some(&changes), output)
}

pub fn command_fragment(
    library: &Library,
    args: &FragmentArgs,
    output: OutputMode,
    config: &AppConfig,
) -> Result<()> {
    let (snippet, changes) = match &args.command {
        FragmentCommand::Add(args) => add_fragment(
            library,
            &args.selector,
            &FragmentAddOptions {
                title: args.title.clone(),
                language: args
                    .language
                    .clone()
                    .or_else(|| config.default_language.clone())
                    .unwrap_or_else(|| "text".to_owned()),
                source_language: None,
                content: read_optional_text(args.content.as_deref(), args.content_file.as_deref())?
                    .unwrap_or_default(),
                note: read_optional_text(args.note.as_deref(), args.note_file.as_deref())?,
                if_hash: fingerprint(args.optimistic.if_hash.as_deref()),
                force: args.optimistic.force,
                ..FragmentAddOptions::default()
            },
        )?,
        FragmentCommand::Edit(args) => edit_fragment(
            library,
            &args.selector,
            &args.fragment,
            &FragmentEditOptions {
                title: args.title.clone(),
                language: args.language.clone(),
                content: read_optional_text(args.content.as_deref(), args.content_file.as_deref())?,
                note: if args.clear_note {
                    Some(None)
                } else {
                    read_optional_text(args.note.as_deref(), args.note_file.as_deref())?.map(Some)
                },
                if_hash: fingerprint(args.optimistic.if_hash.as_deref()),
                force: args.optimistic.force,
            },
        )?,
        FragmentCommand::Remove(args) => remove_fragment(
            library,
            &args.selector,
            &args.fragment,
            fingerprint(args.optimistic.if_hash.as_deref()).as_ref(),
            args.optimistic.force,
        )?,
        FragmentCommand::Reorder(args) => reorder_fragment(
            library,
            &args.selector,
            &args.fragment,
            args.position,
            fingerprint(args.optimistic.if_hash.as_deref()).as_ref(),
            args.optimistic.force,
        )?,
    };
    print_mutation(&snippet, Some(&changes), output)
}

pub fn command_delete(library: &Library, args: &DeleteArgs, output: OutputMode) -> Result<()> {
    let entry = delete_snippet(
        library,
        &args.selector,
        fingerprint(args.optimistic.if_hash.as_deref()).as_ref(),
        args.optimistic.force,
    )?;
    if output == OutputMode::Human {
        println!("moved to trash: {} ({})", entry.title, entry.entry_id);
    } else {
        print_record(&entry, output)?;
    }
    Ok(())
}

fn edit_external(
    library: &Library,
    args: &EditArgs,
    output: OutputMode,
    config: &AppConfig,
) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(SnipError::usage(
            "external editing requires an interactive terminal; pass a structured change instead, such as --content, --content-file, --title, or --tag",
        ));
    }
    let catalog = library.scan()?;
    let original = match library.resolve_snippet(&catalog, &args.selector) {
        Ok(snippet) => {
            if args.create && edit_has_creation_options(args) {
                return Err(SnipError::usage(
                    "--folder, --tag, and --language are only used when --create creates a missing snippet",
                ));
            }
            snippet.clone()
        }
        // `--create` depends on the resolver contract that only ambiguity carries a hint.
        // Adding a hint to ordinary not-found errors would silently disable creation.
        Err(error) if args.create && error.kind == ErrorKind::NotFound && error.hint.is_none() => {
            create_external_snippet(library, &catalog, args, config)?
        }
        Err(error) => return Err(error),
    };
    let expected = fingerprint(args.optimistic.if_hash.as_deref())
        .unwrap_or_else(|| original.fingerprint.clone());
    if expected != original.fingerprint {
        return Err(SnipError::conflict(format!(
            "snippet changed since it was read: expected {expected}, found {}",
            original.fingerprint
        )));
    }
    let (initial, suffix, target, fragment_path, note_relative_path) = if args.metadata_editor {
        (
            fs::read_to_string(original.package_path.join("snippet.toml"))?,
            ".toml".to_owned(),
            ExternalTarget::Metadata,
            None,
            None,
        )
    } else if args.readme_editor {
        (
            original.readme.clone().unwrap_or_default(),
            ".md".to_owned(),
            ExternalTarget::Readme,
            None,
            None,
        )
    } else {
        let fragment = library.resolve_fragment(&original, args.fragment.as_deref())?;
        if args.note_editor {
            (
                fragment.note_content.clone().unwrap_or_default(),
                ".md".to_owned(),
                ExternalTarget::Note(fragment.id.to_string()),
                None,
                fragment.note.clone(),
            )
        } else {
            let suffix = Path::new(&fragment.file)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| format!(".{value}"))
                .unwrap_or_default();
            (
                fragment.content.clone(),
                suffix,
                ExternalTarget::Content(fragment.id.to_string()),
                Some(fragment.absolute_path.clone()),
                None,
            )
        }
    };
    let leaf_dir = external_target_dir(
        &original.package_path,
        fragment_path.as_deref(),
        note_relative_path.as_deref(),
        &target,
    );
    let cwd = resolve_editor_cwd(
        library.root(),
        &original.package_path,
        leaf_dir.as_deref(),
        config.editor_cwd.unwrap_or_default(),
    );
    let mut temp = Builder::new()
        .prefix("snip-edit-")
        .suffix(&suffix)
        .tempfile()?;
    temp.write_all(initial.as_bytes())?;
    temp.as_file().sync_all()?;
    launch_editor(temp.path(), cwd.as_deref(), config.editor.as_deref())?;
    let edited = fs::read_to_string(temp.path())?;
    if edited == initial {
        if output == OutputMode::Human {
            println!("unchanged: {}", original.id);
        } else {
            print_record(&json!({"unchanged": true, "id": original.id}), output)?;
        }
        return Ok(());
    }
    let fresh_catalog = library.scan()?;
    let fresh = library.resolve_snippet(&fresh_catalog, &original.id.to_string())?;
    if fresh.fingerprint != original.fingerprint {
        return Err(SnipError::conflict(format!(
            "snippet changed while the editor was open: expected {}, found {}",
            original.fingerprint, fresh.fingerprint
        )));
    }
    let (snippet, changes) = match target {
        ExternalTarget::Metadata => replace_manifest_text(
            library,
            &original.id.to_string(),
            &edited,
            Some(&expected),
            args.optimistic.force,
        )?,
        ExternalTarget::Readme => edit_snippet(
            library,
            &original.id.to_string(),
            &EditOptions {
                readme: Some(Some(edited)),
                if_hash: Some(expected),
                force: args.optimistic.force,
                ..EditOptions::default()
            },
        )?,
        ExternalTarget::Content(fragment) => edit_snippet(
            library,
            &original.id.to_string(),
            &EditOptions {
                fragment_selector: Some(fragment),
                content: Some(edited),
                if_hash: Some(expected),
                force: args.optimistic.force,
                ..EditOptions::default()
            },
        )?,
        ExternalTarget::Note(fragment) => edit_snippet(
            library,
            &original.id.to_string(),
            &EditOptions {
                fragment_selector: Some(fragment),
                note: Some(Some(edited)),
                if_hash: Some(expected),
                force: args.optimistic.force,
                ..EditOptions::default()
            },
        )?,
    };
    print_mutation(&snippet, Some(&changes), output)
}

fn create_external_snippet(
    library: &Library,
    catalog: &CatalogSnapshot,
    args: &EditArgs,
    config: &AppConfig,
) -> Result<Snippet> {
    let (selector_folder, title) = split_create_selector(&args.selector)?;
    let folder = selector_folder
        .or_else(|| args.folder.clone())
        .or_else(|| config.default_folder.clone());
    if let Some(snippet) = find_existing_create_snippet(
        &catalog.snippets,
        folder.as_deref().unwrap_or_default(),
        &title,
    )? {
        return Ok(snippet.clone());
    }
    let tags = if args.tags.is_empty() {
        config.default_tags.clone()
    } else {
        args.tags.clone()
    };
    create_snippet(
        library,
        &CreateOptions {
            title,
            folder,
            tags,
            language: args
                .language
                .clone()
                .or_else(|| config.default_language.clone())
                .unwrap_or_else(|| "text".to_owned()),
            ..CreateOptions::default()
        },
    )
}

fn find_existing_create_snippet<'a>(
    snippets: &'a [Snippet],
    folder: &str,
    title: &str,
) -> Result<Option<&'a Snippet>> {
    let mut matches = snippets
        .iter()
        .filter(|snippet| snippet.folder == folder && snippet.title == title)
        .collect::<Vec<_>>();
    match matches.as_mut_slice() {
        [] => Ok(None),
        [snippet] => Ok(Some(*snippet)),
        _ => {
            matches.sort_by(|left, right| left.package_path.cmp(&right.package_path));
            let count = matches.len();
            let candidates = matches
                .iter()
                .take(10)
                .map(|snippet| {
                    let package = snippet
                        .package_path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| snippet.package_path.to_string_lossy().into_owned());
                    let path = if snippet.folder.is_empty() {
                        package
                    } else {
                        format!("{}/{package}", snippet.folder)
                    };
                    (path, snippet.folder.as_str())
                })
                .collect::<Vec<_>>();
            let width = candidates
                .iter()
                .map(|(path, _)| path.len())
                .max()
                .unwrap_or_default();
            let mut hint = String::from("pass a package path to pick one:");
            for (path, folder) in candidates {
                if folder.is_empty() {
                    write!(hint, "\n  {path:<width$}  (uncategorized)")
                        .expect("writing to a String cannot fail");
                } else {
                    write!(hint, "\n  {path:<width$}  (folder: {folder})")
                        .expect("writing to a String cannot fail");
                }
            }
            if count > 10 {
                write!(hint, "\n  … and {} more", count - 10)
                    .expect("writing to a String cannot fail");
            }
            Err(SnipError::not_found(format!(
                "ambiguous snippet title {title:?}: {count} matches"
            ))
            .with_hint(hint))
        }
    }
}

fn split_create_selector(selector: &str) -> Result<(Option<String>, String)> {
    let Some((folder, title)) = selector.rsplit_once('/') else {
        return Ok((None, selector.to_owned()));
    };
    if title.is_empty() {
        return Err(SnipError::usage(
            "cannot create a snippet from a selector with an empty title",
        ));
    }
    Ok((Some(folder.to_owned()), title.to_owned()))
}

enum ExternalTarget {
    Metadata,
    Readme,
    Content(String),
    Note(String),
}

fn external_target_dir(
    package_path: &Path,
    fragment_path: Option<&Path>,
    note_relative_path: Option<&str>,
    target: &ExternalTarget,
) -> Option<PathBuf> {
    editor_dir_for_target(
        package_path,
        fragment_path,
        note_relative_path,
        external_target_kind(target),
    )
}

fn external_target_kind(target: &ExternalTarget) -> EditorTargetKind {
    match target {
        ExternalTarget::Metadata => EditorTargetKind::Metadata,
        ExternalTarget::Readme => EditorTargetKind::Readme,
        ExternalTarget::Content(_) => EditorTargetKind::Content,
        ExternalTarget::Note(_) => EditorTargetKind::Note,
    }
}

fn read_optional_text(inline: Option<&str>, path: Option<&str>) -> Result<Option<String>> {
    match inline {
        Some(value) => Ok(Some(value.to_owned())),
        None => read_optional_file(path),
    }
}

fn read_optional_file(path: Option<&str>) -> Result<Option<String>> {
    let Some(path) = path else {
        return Ok(None);
    };
    if path == "-" {
        let mut value = String::new();
        io::stdin().read_to_string(&mut value)?;
        return Ok(Some(value));
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|error| SnipError::io(format!("cannot read {path:?}: {error}")))
}

fn edit_has_create_incompatible_changes(args: &EditArgs) -> bool {
    args.title.is_some()
        || args.clear_tags
        || args.pin
        || args.unpin
        || args.lock
        || args.unlock
        || args.fragment_title.is_some()
        || args.content.is_some()
        || args.content_file.is_some()
        || args.note.is_some()
        || args.note_file.is_some()
        || args.clear_note
        || args.readme.is_some()
        || args.readme_file.is_some()
        || args.clear_readme
}

fn edit_has_creation_options(args: &EditArgs) -> bool {
    args.folder.is_some() || !args.tags.is_empty() || args.language.is_some()
}

fn edit_has_structured_changes(args: &EditArgs) -> bool {
    args.title.is_some()
        || args.folder.is_some()
        || !args.tags.is_empty()
        || args.clear_tags
        || args.pin
        || args.unpin
        || args.lock
        || args.unlock
        || args.fragment_title.is_some()
        || args.language.is_some()
        || args.content.is_some()
        || args.content_file.is_some()
        || args.note.is_some()
        || args.note_file.is_some()
        || args.clear_note
        || args.readme.is_some()
        || args.readme_file.is_some()
        || args.clear_readme
}

fn fingerprint(value: Option<&str>) -> Option<Fingerprint> {
    value.map(|value| Fingerprint(value.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{
        EditorTargetKind, ExternalTarget, external_target_kind, find_existing_create_snippet,
        split_create_selector,
    };
    use snip::domain::{Fingerprint, Snippet, SnippetManifest};
    use snip::error::ErrorKind;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn external_edit_targets_map_to_shared_target_kinds() {
        assert_eq!(
            external_target_kind(&ExternalTarget::Metadata),
            EditorTargetKind::Metadata
        );
        assert_eq!(
            external_target_kind(&ExternalTarget::Readme),
            EditorTargetKind::Readme
        );
        assert_eq!(
            external_target_kind(&ExternalTarget::Content(String::new())),
            EditorTargetKind::Content
        );
        assert_eq!(
            external_target_kind(&ExternalTarget::Note(String::new())),
            EditorTargetKind::Note
        );
    }

    #[test]
    fn create_lookup_matches_nested_folder_and_title() {
        let snippets = vec![
            snippet("scratch", "notes", "notes--11111111"),
            snippet("archive", "notes", "notes--22222222"),
        ];

        let found = find_existing_create_snippet(&snippets, "scratch", "notes")
            .unwrap()
            .unwrap();

        assert_eq!(
            found.package_path,
            PathBuf::from("snippets/scratch/notes--11111111")
        );
    }

    #[test]
    fn create_lookup_matches_flat_selector_with_resolved_folder() {
        let snippets = vec![snippet("default", "notes", "notes--11111111")];

        let found = find_existing_create_snippet(&snippets, "default", "notes")
            .unwrap()
            .unwrap();

        assert_eq!(found.title, "notes");
    }

    #[test]
    fn create_lookup_matches_uncategorized_empty_folder() {
        let snippets = vec![snippet("", "notes", "notes--11111111")];

        let found = find_existing_create_snippet(&snippets, "", "notes")
            .unwrap()
            .unwrap();

        assert!(found.folder.is_empty());
    }

    #[test]
    fn create_lookup_rejects_duplicate_folder_and_title() {
        let snippets = vec![
            snippet("scratch", "notes", "notes--11111111"),
            snippet("scratch", "notes", "notes--22222222"),
        ];

        let error = find_existing_create_snippet(&snippets, "scratch", "notes").unwrap_err();

        assert_eq!(error.kind, ErrorKind::NotFound);
        assert_eq!(
            error.message,
            "ambiguous snippet title \"notes\": 2 matches"
        );
        assert_eq!(
            error.hint.as_deref(),
            Some(
                "pass a package path to pick one:\n  scratch/notes--11111111  (folder: scratch)\n  scratch/notes--22222222  (folder: scratch)"
            )
        );
    }

    #[test]
    fn create_lookup_returns_none_for_missing_snippet() {
        let snippets = vec![snippet("scratch", "other", "other--11111111")];

        let found = find_existing_create_snippet(&snippets, "scratch", "notes").unwrap();

        assert!(found.is_none());
    }

    #[test]
    fn create_selector_splits_nested_folders_and_rejects_trailing_slashes() {
        assert_eq!(
            split_create_selector("foo/bar").unwrap(),
            (Some("foo".to_owned()), "bar".to_owned())
        );
        assert_eq!(
            split_create_selector("a/b/c").unwrap(),
            (Some("a/b".to_owned()), "c".to_owned())
        );
        assert_eq!(
            split_create_selector("title").unwrap(),
            (None, "title".to_owned())
        );
        assert!(split_create_selector("foo/").is_err());
    }

    fn snippet(folder: &str, title: &str, package: &str) -> Snippet {
        let package_path = if folder.is_empty() {
            PathBuf::from("snippets").join(package)
        } else {
            PathBuf::from("snippets").join(folder).join(package)
        };
        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::nil(),
                title: title.to_owned(),
                tags: Vec::new(),
                pinned: false,
                locked: false,
                created_at: String::new(),
                source: None,
                remotes: Vec::new(),
                fragments: Vec::new(),
                extra: toml::Table::new(),
            },
            readme: None,
            folder: folder.to_owned(),
            package_path,
            modified_at: None,
            fingerprint: Fingerprint(String::new()),
            loaded_fragments: Vec::new(),
        }
    }
}
