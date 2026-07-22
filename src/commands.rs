use clap::CommandFactory;
use serde::Serialize;
use serde_json::json;
use snip::config::{AppConfig, ColorSetting, OutputSetting, PreviewRenderSetting, config_path};
use snip::domain::Fingerprint;
use snip::error::{Result, SnipError};
use snip::importer::import_snippetslab;
use snip::render::{RenderMode, preview};
use snip::search::{MemoryIndex, SearchIndex};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, FragmentEditOptions, add_fragment,
    create_folder, create_snippet, delete_folder, delete_snippet, delete_tag, doctor,
    edit_fragment, edit_snippet, move_folder, organize, purge_snippet, remove_fragment, rename_tag,
    reorder_fragment, replace_manifest_text, restore_snippet, trash_entries,
};
use snip::{CatalogSnapshot, Library, Snippet};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use tempfile::Builder;

use crate::cli::*;

pub fn run(cli: &Cli) -> Result<()> {
    if let Some(Command::Completion(args)) = &cli.command {
        return command_completion(args);
    }
    if let Some(Command::Config(args)) = &cli.command {
        return command_config(args, cli.output);
    }
    let config = AppConfig::load()?;
    let output = resolve_output(cli.output, &config);
    let color = resolve_color(cli.color, &config);
    if cli.command.is_none() {
        #[cfg(feature = "tui")]
        {
            if io::stdin().is_terminal() && io::stdout().is_terminal() {
                let path =
                    Library::discover(cli.library.as_deref(), config.default_library.as_deref())?;
                return snip::tui::run(Library::open(&path)?, &config);
            }
        }
        return Err(SnipError::usage(
            "a command is required when stdin or stdout is not a terminal; try --help",
        ));
    }
    match cli.command.as_ref() {
        Some(Command::Init(args)) => return command_init(args, output),
        Some(Command::Import(args)) => return command_import(args, output),
        _ => {}
    }
    let path = Library::discover(cli.library.as_deref(), config.default_library.as_deref())?;
    let library = Library::open(&path)?;
    let command = cli.command.as_ref().expect("command checked above");
    match command {
        #[cfg(feature = "tui")]
        Command::Tui => snip::tui::run(library, &config),
        Command::Info => command_info(&library, output),
        Command::List(args) => command_list(&library, args, output),
        Command::Search(args) => command_search(&library, args, output),
        Command::Show(args) => command_show(&library, args, output),
        Command::Cat(args) => command_cat(&library, args),
        Command::Preview(args) => command_preview(&library, args, color, &config),
        Command::Path(args) => command_path(&library, args),
        Command::Create(args) => command_create(&library, args, output, &config),
        Command::Edit(args) => command_edit(&library, args, output, &config),
        Command::Fragment(args) => command_fragment(&library, args, output, &config),
        Command::Folder(args) => command_folder(&library, args, output),
        Command::Tag(args) => command_tag(&library, args, output),
        Command::Delete(args) => command_delete(&library, args, output),
        Command::Trash => command_trash(&library, output),
        Command::Restore(args) => command_restore(&library, args, output),
        Command::Purge(args) => command_purge(&library, args, output),
        Command::Doctor(args) => command_doctor(&library, args, output),
        Command::Organize(args) => command_organize(&library, args, output),
        Command::Git(args) => command_git(&library, args),
        Command::Config(_) | Command::Init(_) | Command::Import(_) | Command::Completion(_) => {
            unreachable!()
        }
    }
}

pub fn effective_output(cli: &Cli) -> OutputMode {
    cli.output.unwrap_or_else(|| {
        AppConfig::load()
            .ok()
            .as_ref()
            .map_or(OutputMode::Human, |config| resolve_output(None, config))
    })
}

fn resolve_output(explicit: Option<OutputMode>, config: &AppConfig) -> OutputMode {
    explicit.unwrap_or(match config.output {
        Some(OutputSetting::Json) => OutputMode::Json,
        Some(OutputSetting::Jsonl) => OutputMode::Jsonl,
        Some(OutputSetting::Human) | None => OutputMode::Human,
    })
}

fn resolve_color(explicit: Option<ColorMode>, config: &AppConfig) -> ColorMode {
    explicit.unwrap_or(match config.color {
        Some(ColorSetting::Always) => ColorMode::Always,
        Some(ColorSetting::Never) => ColorMode::Never,
        Some(ColorSetting::Auto) | None => ColorMode::Auto,
    })
}

fn command_init(args: &InitArgs, output: OutputMode) -> Result<()> {
    let library = Library::init(&args.path, args.name.as_deref())?;
    if args.git {
        run_process(
            ProcessCommand::new("git")
                .arg("init")
                .current_dir(library.root()),
            "git init",
        )?;
    }
    let value = json!({
        "path": library.root(),
        "id": library.manifest().id,
        "name": library.manifest().name,
        "schema_version": library.manifest().schema_version,
        "git_initialized": args.git,
    });
    if output == OutputMode::Human {
        println!("initialized: {}", library.root().display());
        println!("library id: {}", library.manifest().id);
    } else {
        print_record(&value, output)?;
    }
    Ok(())
}

fn command_info(library: &Library, output: OutputMode) -> Result<()> {
    let catalog = library.scan()?;
    let value = json!({
        "path": library.root(),
        "format": catalog.library.format,
        "schema_version": catalog.library.schema_version,
        "id": catalog.library.id,
        "name": catalog.library.name,
        "created_at": catalog.library.created_at,
        "snippets": catalog.snippets.len(),
        "folders": catalog.folders.len(),
        "tags": catalog.tags.len(),
        "fragments": catalog.snippets.iter().map(|snippet| snippet.loaded_fragments.len()).sum::<usize>(),
        "trash": trash_entries(library)?.len(),
    });
    if output == OutputMode::Human {
        for key in [
            "path",
            "name",
            "id",
            "format",
            "schema_version",
            "snippets",
            "fragments",
            "folders",
            "tags",
            "trash",
        ] {
            println!("{key}: {}", display_json_scalar(&value[key]));
        }
    } else {
        print_record(&value, output)?;
    }
    Ok(())
}

fn command_list(library: &Library, args: &FilterArgs, output: OutputMode) -> Result<()> {
    let catalog = library.scan()?;
    let snippets = filter_snippets(&catalog, args.folder.as_deref(), args.tag.as_deref());
    let rows = snippets.iter().map(snippet_summary).collect::<Vec<_>>();
    if output == OutputMode::Human {
        for snippet in snippets {
            println!(
                "{}  {}  [{}]  {}",
                &snippet.id.simple().to_string()[..8],
                snippet.title,
                if snippet.folder.is_empty() {
                    "Uncategorized"
                } else {
                    &snippet.folder
                },
                snippet.tags.join(", ")
            );
        }
    } else {
        print_records(&rows, output)?;
    }
    Ok(())
}

fn command_search(library: &Library, args: &SearchArgs, output: OutputMode) -> Result<()> {
    let index = MemoryIndex::new(library.scan()?);
    let results = index.search(&args.query, args.folder.as_deref(), args.tag.as_deref());
    if output == OutputMode::Human {
        for result in results {
            let location = result
                .line
                .map(|line| format!(":{line}"))
                .unwrap_or_default();
            println!(
                "{}{}  {}  {}",
                &result.snippet_id.simple().to_string()[..8],
                location,
                result.title,
                result.excerpt
            );
        }
    } else {
        print_records(&results, output)?;
    }
    Ok(())
}

fn command_show(library: &Library, args: &SelectorArgs, output: OutputMode) -> Result<()> {
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, &args.selector)?;
    if output == OutputMode::Human {
        println!("ID: {}", snippet.id);
        println!("Title: {}", snippet.title);
        println!(
            "Folder: {}",
            if snippet.folder.is_empty() {
                "Uncategorized"
            } else {
                &snippet.folder
            }
        );
        println!("Tags: {}", snippet.tags.join(", "));
        println!("Created: {}", snippet.created_at);
        println!(
            "Modified: {}",
            snippet.modified_at.as_deref().unwrap_or("-")
        );
        println!("Pinned: {}  Locked: {}", snippet.pinned, snippet.locked);
        println!("Fingerprint: {}", snippet.fingerprint);
        for (index, fragment) in snippet.loaded_fragments.iter().enumerate() {
            println!(
                "\n--- {}. {} ({}) [{}] ---",
                index + 1,
                fragment.title,
                fragment.language,
                fragment.id
            );
            print!("{}", fragment.content);
            if !fragment.content.ends_with('\n') {
                println!();
            }
            if let Some(note) = &fragment.note_content {
                println!("\nNote:\n{note}");
            }
        }
    } else {
        print_record(snippet, output)?;
    }
    Ok(())
}

fn command_cat(library: &Library, args: &FragmentSelectorArgs) -> Result<()> {
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, &args.selector)?;
    let fragment = library.resolve_fragment(snippet, args.fragment.as_deref())?;
    print!("{}", fragment.content);
    io::stdout().flush()?;
    Ok(())
}

fn command_preview(
    library: &Library,
    args: &PreviewArgs,
    color: ColorMode,
    config: &AppConfig,
) -> Result<()> {
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, &args.selector)?;
    let render = args.render.unwrap_or(match config.preview_render {
        Some(PreviewRenderSetting::Plain) => RenderArg::Plain,
        Some(PreviewRenderSetting::Html) => RenderArg::Html,
        Some(PreviewRenderSetting::Ansi) | None => RenderArg::Ansi,
    });
    let mode = match render {
        RenderArg::Ansi => RenderMode::Ansi,
        RenderArg::Plain => RenderMode::Plain,
        RenderArg::Html => RenderMode::Html,
    };
    let use_color = match color {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => io::stdout().is_terminal(),
    };
    let rendered = preview(snippet, mode, use_color)?;
    let use_pager = if args.pager {
        true
    } else if args.no_pager {
        false
    } else {
        config.preview_pager.unwrap_or(false)
    };
    if use_pager {
        send_to_pager(&rendered, config.pager.as_deref())?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn command_path(library: &Library, args: &PathArgs) -> Result<()> {
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, &args.selector)?;
    let path = if args.metadata {
        snippet.package_path.join("snippet.toml")
    } else if args.readme {
        snippet.package_path.join("README.md")
    } else if let Some(fragment) = &args.fragment {
        library
            .resolve_fragment(snippet, Some(fragment))?
            .absolute_path
            .clone()
    } else {
        snippet.package_path.clone()
    };
    println!("{}", path.display());
    Ok(())
}

fn command_create(
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
            content: read_optional_file(args.content_file.as_deref())?.unwrap_or_default(),
            note: read_optional_file(args.note_file.as_deref())?,
            readme: read_optional_file(args.readme_file.as_deref())?,
            pinned: args.pin,
            locked: args.lock,
            ..CreateOptions::default()
        },
    )?;
    print_mutation(&snippet, None, output)
}

fn command_edit(
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
        content: read_optional_file(args.content_file.as_deref())?,
        note: if args.clear_note {
            Some(None)
        } else {
            read_optional_file(args.note_file.as_deref())?.map(Some)
        },
        readme: if args.clear_readme {
            Some(None)
        } else {
            read_optional_file(args.readme_file.as_deref())?.map(Some)
        },
        if_hash: fingerprint(args.optimistic.if_hash.as_deref()),
        force: args.optimistic.force,
    };
    let (snippet, changes) = edit_snippet(library, &args.selector, &options)?;
    print_mutation(&snippet, Some(&changes), output)
}

fn command_fragment(
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
                content: read_optional_file(args.content_file.as_deref())?.unwrap_or_default(),
                note: read_optional_file(args.note_file.as_deref())?,
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
                content: read_optional_file(args.content_file.as_deref())?,
                note: if args.clear_note {
                    Some(None)
                } else {
                    read_optional_file(args.note_file.as_deref())?.map(Some)
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

fn command_folder(library: &Library, args: &FolderArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        FolderCommand::List => {
            let folders = library.scan()?.folders;
            if output == OutputMode::Human {
                for folder in folders {
                    println!("{folder}");
                }
            } else {
                print_records(&folders, output)?;
            }
        }
        FolderCommand::Create { folder } => {
            let path = create_folder(library, folder)?;
            print_simple_path("created", &path, output)?;
        }
        FolderCommand::Rename { folder, new_name } => {
            if Path::new(new_name).components().count() != 1 {
                return Err(SnipError::usage(
                    "new folder name must be one path component",
                ));
            }
            let source = Path::new(folder);
            let target = source
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(new_name);
            let path = move_folder(library, folder, &target.to_string_lossy())?;
            print_simple_path("renamed", &path, output)?;
        }
        FolderCommand::Move { folder, target } => {
            let path = move_folder(library, folder, target)?;
            print_simple_path("moved", &path, output)?;
        }
        FolderCommand::Delete { folder } => {
            delete_folder(library, folder)?;
            if output == OutputMode::Human {
                println!("deleted folder: {folder}");
            } else {
                print_record(&json!({"deleted": folder}), output)?;
            }
        }
    }
    Ok(())
}

fn command_tag(library: &Library, args: &TagArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        TagCommand::List => {
            let tags = library.scan()?.tags;
            if output == OutputMode::Human {
                for tag in tags {
                    println!("{tag}");
                }
            } else {
                print_records(&tags, output)?;
            }
        }
        TagCommand::Rename { tag, new_name } => {
            let changed = rename_tag(library, tag, new_name)?;
            print_count("updated_snippets", changed, output)?;
        }
        TagCommand::Delete { tag } => {
            let changed = delete_tag(library, tag)?;
            print_count("updated_snippets", changed, output)?;
        }
    }
    Ok(())
}

fn command_delete(library: &Library, args: &DeleteArgs, output: OutputMode) -> Result<()> {
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

fn command_trash(library: &Library, output: OutputMode) -> Result<()> {
    let entries = trash_entries(library)?;
    if output == OutputMode::Human {
        for entry in entries {
            println!(
                "{}  {}  {}  {}",
                &entry.entry_id[..8],
                entry.deleted_at,
                entry.title,
                entry.original_path
            );
        }
    } else {
        print_records(&entries, output)?;
    }
    Ok(())
}

fn command_restore(library: &Library, args: &RestoreArgs, output: OutputMode) -> Result<()> {
    let snippet = restore_snippet(library, &args.selector, args.folder.as_deref())?;
    print_mutation(&snippet, None, output)
}

fn command_purge(library: &Library, args: &PurgeArgs, output: OutputMode) -> Result<()> {
    if !args.yes {
        return Err(SnipError::usage(
            "purge permanently deletes data; repeat with --yes",
        ));
    }
    let entry = purge_snippet(library, &args.selector)?;
    if output == OutputMode::Human {
        println!("permanently deleted: {} ({})", entry.title, entry.entry_id);
    } else {
        print_record(&entry, output)?;
    }
    Ok(())
}

fn command_doctor(library: &Library, args: &DoctorArgs, output: OutputMode) -> Result<()> {
    let report = doctor(library, args.repair);
    if output == OutputMode::Human {
        println!("checked: {} snippets", report.checked);
        println!("errors: {}", report.errors.len());
        println!("warnings: {}", report.warnings.len());
        println!(
            "pending transactions: {}",
            report.pending_transactions.len()
        );
        for message in &report.repaired {
            println!("REPAIRED: {message}");
        }
        for message in &report.errors {
            println!("ERROR: {message}");
        }
        for message in &report.warnings {
            println!("WARNING: {message}");
        }
    } else {
        print_record(&report, output)?;
    }
    if !report.ok {
        return Err(SnipError::validation("library validation failed"));
    }
    Ok(())
}

fn command_organize(library: &Library, args: &OrganizeArgs, output: OutputMode) -> Result<()> {
    let changes = organize(library, args.dry_run)?;
    if output == OutputMode::Human {
        for change in &changes {
            println!(
                "{}{} -> {}",
                if args.dry_run {
                    "would move: "
                } else {
                    "moved: "
                },
                change
                    .old_path
                    .as_deref()
                    .unwrap_or_else(|| Path::new("-"))
                    .display(),
                change
                    .new_path
                    .as_deref()
                    .unwrap_or_else(|| Path::new("-"))
                    .display()
            );
        }
        println!("changes: {}", changes.len());
    } else {
        print_records(&changes, output)?;
    }
    Ok(())
}

fn command_import(args: &ImportArgs, output: OutputMode) -> Result<()> {
    match &args.command {
        ImportCommand::Snippetslab {
            source,
            into,
            dry_run,
        } => {
            let report = import_snippetslab(source, into, *dry_run)?;
            if output == OutputMode::Human {
                println!("source: {}", report.source.display());
                println!("destination: {}", report.destination.display());
                println!("dry run: {}", report.dry_run);
                println!("snippets: {}", report.snippets);
                println!("folders: {}", report.folders);
                println!("tags: {}", report.tags);
                println!("fragments: {}", report.fragments);
                println!("notes: {}", report.notes);
                println!("attachments: {}", report.attachments);
                for item in report.normalized_tags {
                    println!("NORMALIZED TAG: {item}");
                }
                for item in report.warnings {
                    println!("WARNING: {item}");
                }
            } else {
                print_record(&report, output)?;
            }
        }
    }
    Ok(())
}

fn command_git(library: &Library, args: &GitArgs) -> Result<()> {
    match &args.command {
        GitCommand::Status => stream_git(library, &["status", "--short", "--", "."]),
        GitCommand::Diff => stream_git(library, &["diff", "--", "."]),
        GitCommand::Log { limit } => stream_git(
            library,
            &["log", "--oneline", &format!("-{limit}"), "--", "."],
        ),
        GitCommand::Commit { message } => {
            let top = git_output(library, &["rev-parse", "--show-toplevel"])?;
            let top = fs::canonicalize(top.trim()).map_err(|error| {
                SnipError::io(format!("cannot resolve Git root {:?}: {error}", top.trim()))
            })?;
            if top != library.root() {
                return Err(SnipError::conflict(
                    "snip git commit is allowed only when the library root is the Git root; use Git directly for nested libraries",
                ));
            }
            stream_git(
                library,
                &[
                    "add",
                    "--",
                    "snip.toml",
                    "tags.toml",
                    "snippets",
                    "trash",
                    ".gitignore",
                ],
            )?;
            stream_git(library, &["commit", "-m", message])
        }
    }
}

fn command_config(args: &ConfigArgs, explicit_output: Option<OutputMode>) -> Result<()> {
    let path = config_path()?;
    match &args.command {
        ConfigCommand::Path => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigCommand::Init { library, force } => {
            if path.exists() && !force {
                return Err(SnipError::conflict(format!(
                    "config already exists: {}; pass --force to replace it",
                    path.display()
                )));
            }
            let mut config = AppConfig {
                output: Some(OutputSetting::Human),
                color: Some(ColorSetting::Auto),
                preview_render: Some(PreviewRenderSetting::Ansi),
                preview_pager: Some(false),
                default_language: Some("text".to_owned()),
                ..AppConfig::default()
            };
            if let Some(library) = library {
                config.default_library = Some(validated_library_path(library)?);
            }
            config.save_to(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
        ConfigCommand::Show => {
            let config = AppConfig::load_from(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
        ConfigCommand::Set { key, value } => {
            let mut config = AppConfig::load_from(&path)?;
            set_config_value(&mut config, *key, value)?;
            config.save_to(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
        ConfigCommand::Unset { key } => {
            let mut config = AppConfig::load_from(&path)?;
            unset_config_value(&mut config, *key);
            config.save_to(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
    }
}

fn print_config(config: &AppConfig, path: &Path, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("config: {}", path.display());
        print!("{}", toml::to_string_pretty(config)?);
        Ok(())
    } else {
        print_record(&json!({"path": path, "config": config}), output)
    }
}

fn set_config_value(config: &mut AppConfig, key: ConfigKey, value: &str) -> Result<()> {
    match key {
        ConfigKey::DefaultLibrary => {
            config.default_library = Some(validated_library_path(&expand_user_path(value)?)?);
        }
        ConfigKey::Output => {
            config.output = Some(match value.to_ascii_lowercase().as_str() {
                "human" => OutputSetting::Human,
                "json" => OutputSetting::Json,
                "jsonl" => OutputSetting::Jsonl,
                _ => return Err(SnipError::usage("output must be human, json, or jsonl")),
            });
        }
        ConfigKey::Color => {
            config.color = Some(match value.to_ascii_lowercase().as_str() {
                "auto" => ColorSetting::Auto,
                "always" => ColorSetting::Always,
                "never" => ColorSetting::Never,
                _ => return Err(SnipError::usage("color must be auto, always, or never")),
            });
        }
        ConfigKey::PreviewRender => {
            config.preview_render = Some(match value.to_ascii_lowercase().as_str() {
                "ansi" => PreviewRenderSetting::Ansi,
                "plain" => PreviewRenderSetting::Plain,
                "html" => PreviewRenderSetting::Html,
                _ => {
                    return Err(SnipError::usage(
                        "preview-render must be ansi, plain, or html",
                    ));
                }
            });
        }
        ConfigKey::PreviewPager => config.preview_pager = Some(parse_bool(value)?),
        ConfigKey::Editor => config.editor = Some(nonempty_value("editor", value)?),
        ConfigKey::Pager => config.pager = Some(nonempty_value("pager", value)?),
        ConfigKey::DefaultLanguage => {
            config.default_language = Some(nonempty_value("default-language", value)?)
        }
        ConfigKey::DefaultFolder => config.default_folder = Some(value.trim().to_owned()),
        ConfigKey::DefaultTags => {
            let tags = value.split(',').map(str::to_owned).collect::<Vec<_>>();
            config.default_tags = snip::filesystem::normalize_tags(&tags)?;
        }
    }
    Ok(())
}

fn unset_config_value(config: &mut AppConfig, key: ConfigKey) {
    match key {
        ConfigKey::DefaultLibrary => config.default_library = None,
        ConfigKey::Output => config.output = None,
        ConfigKey::Color => config.color = None,
        ConfigKey::PreviewRender => config.preview_render = None,
        ConfigKey::PreviewPager => config.preview_pager = None,
        ConfigKey::Editor => config.editor = None,
        ConfigKey::Pager => config.pager = None,
        ConfigKey::DefaultLanguage => config.default_language = None,
        ConfigKey::DefaultFolder => config.default_folder = None,
        ConfigKey::DefaultTags => config.default_tags.clear(),
    }
}

fn validated_library_path(path: &Path) -> Result<PathBuf> {
    Ok(Library::open(path)?.root().to_path_buf())
}

fn expand_user_path(value: &str) -> Result<PathBuf> {
    if value == "~" || value.starts_with("~/") {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| SnipError::io("cannot expand ~: HOME is not set"))?;
        let mut path = PathBuf::from(home);
        if value.len() > 2 {
            path.push(&value[2..]);
        }
        Ok(path)
    } else {
        Ok(PathBuf::from(value))
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(SnipError::usage(
            "boolean value must be true/false, yes/no, on/off, or 1/0",
        )),
    }
}

fn nonempty_value(name: &str, value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        Err(SnipError::usage(format!("{name} cannot be empty")))
    } else {
        Ok(value.to_owned())
    }
}

fn command_completion(args: &CompletionArgs) -> Result<()> {
    let mut command = Cli::command();
    let shell = match args.shell {
        CompletionShell::Bash => clap_complete::Shell::Bash,
        CompletionShell::Elvish => clap_complete::Shell::Elvish,
        CompletionShell::Fish => clap_complete::Shell::Fish,
        CompletionShell::Powershell => clap_complete::Shell::PowerShell,
        CompletionShell::Zsh => clap_complete::Shell::Zsh,
    };
    clap_complete::generate(shell, &mut command, "snip", &mut io::stdout());
    Ok(())
}

fn edit_external(
    library: &Library,
    args: &EditArgs,
    output: OutputMode,
    configured_editor: Option<&str>,
) -> Result<()> {
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

fn send_to_pager(content: &str, configured_pager: Option<&str>) -> Result<()> {
    let pager = configured_pager
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| std::env::var("PAGER").unwrap_or_else(|_| "less -R".to_owned()));
    let parts = shlex::split(&pager)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(|| SnipError::usage(format!("invalid pager command: {pager:?}")))?;
    let mut child = ProcessCommand::new(&parts[0])
        .args(&parts[1..])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| SnipError::io(format!("cannot start pager: {error}")))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| SnipError::io("pager stdin is unavailable"))?
        .write_all(content.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        return Err(SnipError::io(format!("pager exited with status {status}")));
    }
    Ok(())
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
        || args.content_file.is_some()
        || args.note_file.is_some()
        || args.clear_note
        || args.readme_file.is_some()
        || args.clear_readme
}

fn filter_snippets<'a>(
    catalog: &'a CatalogSnapshot,
    folder: Option<&str>,
    tag: Option<&str>,
) -> Vec<&'a Snippet> {
    catalog
        .snippets
        .iter()
        .filter(|snippet| {
            folder.is_none_or(|folder| snippet.folder.eq_ignore_ascii_case(folder))
                && tag.is_none_or(|tag| {
                    snippet
                        .tags
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
                })
        })
        .collect()
}

fn snippet_summary(snippet: &&Snippet) -> serde_json::Value {
    json!({
        "id": snippet.id,
        "title": snippet.title,
        "folder": snippet.folder,
        "tags": snippet.tags,
        "pinned": snippet.pinned,
        "locked": snippet.locked,
        "created_at": snippet.created_at,
        "modified_at": snippet.modified_at,
        "fingerprint": snippet.fingerprint,
        "fragments": snippet.fragments.len(),
        "path": snippet.package_path,
    })
}

fn fingerprint(value: Option<&str>) -> Option<Fingerprint> {
    value.map(|value| Fingerprint(value.to_owned()))
}

fn print_mutation(
    snippet: &Snippet,
    changes: Option<&snip::ChangeSet>,
    output: OutputMode,
) -> Result<()> {
    if output == OutputMode::Human {
        println!("updated: {}", snippet.package_path.display());
        println!("id: {}", snippet.id);
        println!("fingerprint: {}", snippet.fingerprint);
        if let Some(changes) = changes {
            println!("fields: {}", changes.fields.join(", "));
        }
    } else {
        print_record(
            &json!({
                "snippet": snippet,
                "changes": changes,
            }),
            output,
        )?;
    }
    Ok(())
}

fn print_record<T: Serialize>(value: &T, output: OutputMode) -> Result<()> {
    match output {
        OutputMode::Human => unreachable!(),
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(value)?),
        OutputMode::Jsonl => println!("{}", serde_json::to_string(value)?),
    }
    Ok(())
}

fn print_records<T: Serialize>(values: &[T], output: OutputMode) -> Result<()> {
    match output {
        OutputMode::Human => unreachable!(),
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(values)?),
        OutputMode::Jsonl => {
            for value in values {
                println!("{}", serde_json::to_string(value)?);
            }
        }
    }
    Ok(())
}

fn print_simple_path(label: &str, path: &Path, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("{label}: {}", path.display());
    } else {
        print_record(&json!({label: path}), output)?;
    }
    Ok(())
}

fn print_count(label: &str, value: usize, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("{label}: {value}");
    } else {
        print_record(&json!({label: value}), output)?;
    }
    Ok(())
}

fn display_json_scalar(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn run_process(command: &mut ProcessCommand, label: &str) -> Result<()> {
    let status = command
        .status()
        .map_err(|error| SnipError::io(format!("cannot run {label}: {error}")))?;
    if !status.success() {
        return Err(SnipError::io(format!(
            "{label} exited with status {status}"
        )));
    }
    Ok(())
}

fn stream_git(library: &Library, args: &[&str]) -> Result<()> {
    run_process(
        ProcessCommand::new("git")
            .args(args)
            .current_dir(library.root()),
        "git",
    )
}

fn git_output(library: &Library, args: &[&str]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(library.root())
        .output()
        .map_err(|error| SnipError::io(format!("cannot run git: {error}")))?;
    if !output.status.success() {
        return Err(SnipError::io(format!(
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| SnipError::validation(format!("git output is not UTF-8: {error}")))
}
