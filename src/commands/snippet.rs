use serde_json::json;
use snip::Library;
use snip::config::AppConfig;
use snip::domain::Fingerprint;
use snip::error::{Result, SnipError};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, FragmentEditOptions, add_fragment,
    create_snippet, delete_snippet, edit_fragment, edit_snippet, remove_fragment, reorder_fragment,
    replace_manifest_text,
};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use std::process::Command as ProcessCommand;
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
    if !edit_has_structured_changes(args) {
        return edit_external(library, args, output, config.editor.as_deref());
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
    configured_editor: Option<&str>,
) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(SnipError::usage(
            "external editing requires an interactive terminal; pass a structured change instead, such as --content, --content-file, --title, or --tag",
        ));
    }
    let catalog = library.scan()?;
    let original = library.resolve_snippet(&catalog, &args.selector)?.clone();
    let expected = fingerprint(args.optimistic.if_hash.as_deref())
        .unwrap_or_else(|| original.fingerprint.clone());
    if expected != original.fingerprint {
        return Err(SnipError::conflict(format!(
            "snippet changed since it was read: expected {expected}, found {}",
            original.fingerprint
        )));
    }
    let (initial, suffix, target) = if args.metadata_editor {
        (
            fs::read_to_string(original.package_path.join("snippet.toml"))?,
            ".toml".to_owned(),
            ExternalTarget::Metadata,
        )
    } else if args.readme_editor {
        (
            original.readme.clone().unwrap_or_default(),
            ".md".to_owned(),
            ExternalTarget::Readme,
        )
    } else {
        let fragment = library.resolve_fragment(&original, args.fragment.as_deref())?;
        if args.note_editor {
            (
                fragment.note_content.clone().unwrap_or_default(),
                ".md".to_owned(),
                ExternalTarget::Note(fragment.id.to_string()),
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
            )
        }
    };
    let mut temp = Builder::new()
        .prefix("snip-edit-")
        .suffix(&suffix)
        .tempfile()?;
    temp.write_all(initial.as_bytes())?;
    temp.as_file().sync_all()?;
    launch_editor(temp.path(), configured_editor)?;
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

enum ExternalTarget {
    Metadata,
    Readme,
    Content(String),
    Note(String),
}

fn launch_editor(path: &Path, configured_editor: Option<&str>) -> Result<()> {
    let editor = configured_editor.map(ToOwned::to_owned).unwrap_or_else(|| {
        std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_owned())
    });
    let parts = shlex::split(&editor)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(|| SnipError::usage(format!("invalid editor command: {editor:?}")))?;
    let status = ProcessCommand::new(&parts[0])
        .args(&parts[1..])
        .arg(path)
        .status()
        .map_err(|error| SnipError::io(format!("cannot start editor: {error}")))?;
    if !status.success() {
        return Err(SnipError::io(format!("editor exited with status {status}")));
    }
    Ok(())
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
