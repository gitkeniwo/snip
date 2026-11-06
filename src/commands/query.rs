use std::sync::Arc;

use serde_json::json;
use snip::Library;
use snip::budget::{LineRange, cut, human_bytes, select_lines};
use snip::config::{AppConfig, PreviewRenderSetting};
use snip::domain::{CatalogSnapshot, FolderFilter, Snippet, folder_label};
use snip::error::{Result, SnipError};
use snip::render::{RenderMode, preview};
use snip::search::{self, MemoryIndex, SearchIndex, SearchQuery};
use snip::service::trash_entries;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use super::output::{display_json_scalar, print_record, print_records, send_to_pager};
use crate::cli::{
    CatArgs, ColorMode, FilterArgs, OpenArgs, OutputMode, PathArgs, PreviewArgs, RenderArg,
    SearchArgs, ShowArgs,
};

pub fn command_info(library: &Library, output: OutputMode) -> Result<()> {
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

pub fn command_list(library: &Library, args: &FilterArgs, output: OutputMode) -> Result<()> {
    let catalog = library.scan()?;
    let mut snippets = filter_snippets(&catalog, args.folder_filter(), args.tag.as_deref());
    snippets.sort_by(|left, right| args.sort.compare(left, right));
    let rows = snippets
        .iter()
        .map(|snippet| {
            if args.brief {
                snippet_brief(snippet)
            } else {
                snippet_summary(snippet)
            }
        })
        .collect::<Vec<_>>();
    if output == OutputMode::Human {
        for snippet in snippets {
            println!(
                "{}  {}  [{}]  {}",
                &snippet.id.simple().to_string()[..8],
                snippet.title,
                folder_label(&snippet.folder),
                snippet.tags.join(", ")
            );
        }
    } else {
        print_records(&rows, output)?;
    }
    Ok(())
}

pub fn command_search(library: &Library, args: &SearchArgs, output: OutputMode) -> Result<()> {
    let index = MemoryIndex::new(Arc::new(library.scan()?));
    let query = SearchQuery::new(&args.query, args.regex)?
        .folder(args.folder_filter())
        .tag(args.tag.as_deref())
        .fields(&args.fields)
        .context_lines(args.context)
        .limit(if args.brief { None } else { args.limit });
    let results = index.search(&query);
    if args.brief {
        let brief = search::brief(results, index.catalog(), args.limit);
        match output {
            OutputMode::Human => {
                for hit in &brief.results {
                    let matched = hit
                        .matched_fragments
                        .iter()
                        .map(|matched| {
                            if matched.lines.is_empty() {
                                format!("#{}", matched.index)
                            } else {
                                let lines = matched
                                    .lines
                                    .iter()
                                    .map(usize::to_string)
                                    .collect::<Vec<_>>()
                                    .join(",");
                                format!("#{}:{lines}", matched.index)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!(
                        "{}  {}  [{}]  {} frags  {}  {}",
                        &hit.id.simple().to_string()[..8],
                        hit.title,
                        folder_label(&hit.folder),
                        hit.fragments,
                        human_bytes(hit.bytes),
                        matched
                    );
                }
            }
            OutputMode::Json => print_record(&brief, output)?,
            OutputMode::Jsonl => print_records(&brief.results, output)?,
        }
        return Ok(());
    }
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
            if let Some(line) = result.line {
                let first = line.saturating_sub(result.context_before.len());
                for (offset, text) in result.context_before.iter().enumerate() {
                    println!("  {:>5}- {text}", first + offset);
                }
                if !result.context_before.is_empty() || !result.context_after.is_empty() {
                    println!("  {line:>5}: {}", result.excerpt);
                }
                for (offset, text) in result.context_after.iter().enumerate() {
                    println!("  {:>5}- {text}", line + offset + 1);
                }
            }
        }
    } else {
        print_records(&results, output)?;
    }
    Ok(())
}

pub fn command_show(library: &Library, args: &ShowArgs, output: OutputMode) -> Result<()> {
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, &args.selector)?;
    if let Some(budget) = args.budget() {
        return show_budgeted(snippet, budget, output);
    }
    if output == OutputMode::Human {
        println!("ID: {}", snippet.id);
        println!("Title: {}", snippet.title);
        println!("Folder: {}", folder_label(&snippet.folder));
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

/// Spend `remaining` on `text`. Returns what fits; once anything is cut the
/// budget is spent, so later texts are omitted instead of leapfrogging.
fn spend<'a>(text: &'a str, remaining: &mut usize) -> &'a str {
    let shown = cut(text, *remaining);
    if shown.len() < text.len() {
        *remaining = 0;
    } else {
        *remaining -= shown.len();
    }
    shown
}

fn show_budgeted(snippet: &Snippet, budget: usize, output: OutputMode) -> Result<()> {
    let total = snippet.content_bytes();
    let mut remaining = budget;
    let mut emitted = 0usize;

    let readme = snippet.readme.as_deref().map(|readme| {
        let shown = spend(readme, &mut remaining);
        emitted += shown.len();
        shown
    });
    let mut fragments = Vec::new();
    for (index, fragment) in snippet.loaded_fragments.iter().enumerate() {
        let content = spend(&fragment.content, &mut remaining);
        emitted += content.len();
        let note = fragment.note_content.as_deref().map(|note| {
            let shown = spend(note, &mut remaining);
            emitted += shown.len();
            shown
        });
        let truncated = content.len() < fragment.content.len()
            || note.is_some_and(|shown| {
                shown.len() < fragment.note_content.as_ref().map_or(0, String::len)
            });
        fragments.push((index + 1, fragment, content, note, truncated));
    }
    let truncated = emitted < total;

    if output == OutputMode::Human {
        println!("ID: {}", snippet.id);
        println!("Title: {}", snippet.title);
        println!("Folder: {}", folder_label(&snippet.folder));
        println!("Tags: {}", snippet.tags.join(", "));
        println!("Fingerprint: {}", snippet.fingerprint);
        if let Some(readme) = readme.filter(|readme| !readme.is_empty()) {
            println!("\nREADME:\n{readme}");
        }
        for (index, fragment, content, note, _) in &fragments {
            println!(
                "\n--- {index}. {} ({}) [{} bytes, {} lines] ---",
                fragment.title,
                fragment.language,
                fragment.content.len(),
                fragment.content.lines().count()
            );
            print!("{content}");
            if !content.is_empty() && !content.ends_with('\n') {
                println!();
            }
            if let Some(note) = note.filter(|note| !note.is_empty()) {
                println!("\nNote:\n{note}");
            }
        }
        if truncated {
            eprintln!(
                "note: output truncated at {emitted} of {total} bytes (use --fragment / --lines)"
            );
        }
        return Ok(());
    }

    let fragment_rows = fragments
        .iter()
        .map(|(index, fragment, content, note, truncated)| {
            let mut row = serde_json::Map::new();
            row.insert("index".into(), json!(index));
            row.insert("id".into(), json!(fragment.id));
            row.insert("title".into(), json!(fragment.title));
            row.insert("language".into(), json!(fragment.language));
            row.insert("bytes".into(), json!(fragment.content.len()));
            row.insert("lines".into(), json!(fragment.content.lines().count()));
            row.insert("has_note".into(), json!(fragment.note_content.is_some()));
            if !content.is_empty() {
                row.insert("content".into(), json!(content));
            }
            if let Some(full) = &fragment.note_content {
                row.insert("note_bytes".into(), json!(full.len()));
                if note.is_some_and(|shown| !shown.is_empty()) {
                    row.insert("note".into(), json!(note));
                }
            }
            row.insert("truncated".into(), json!(truncated));
            serde_json::Value::Object(row)
        })
        .collect::<Vec<_>>();
    let mut view = serde_json::Map::new();
    view.insert("id".into(), json!(snippet.id));
    view.insert("title".into(), json!(snippet.title));
    view.insert("folder".into(), json!(snippet.folder));
    view.insert("tags".into(), json!(snippet.tags));
    view.insert("fingerprint".into(), json!(snippet.fingerprint));
    view.insert("created_at".into(), json!(snippet.created_at));
    view.insert("modified_at".into(), json!(snippet.modified_at));
    view.insert("pinned".into(), json!(snippet.pinned));
    view.insert("locked".into(), json!(snippet.locked));
    if let Some(full) = &snippet.readme {
        view.insert("readme_bytes".into(), json!(full.len()));
        if readme.is_some_and(|shown| !shown.is_empty()) {
            view.insert("readme".into(), json!(readme));
        }
    }
    view.insert("fragments".into(), json!(fragment_rows));
    view.insert("truncated".into(), json!(truncated));
    view.insert("omitted_bytes".into(), json!(total - emitted));
    print_record(&serde_json::Value::Object(view), output)
}

pub fn command_cat(library: &Library, args: &CatArgs) -> Result<()> {
    let range = args.lines.as_deref().map(LineRange::parse).transpose()?;
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, &args.selector)?;
    let fragment = library.resolve_fragment(snippet, args.fragment.as_deref())?;
    if args.fragment.is_none() && snippet.loaded_fragments.len() > 1 {
        let count = snippet.loaded_fragments.len();
        eprintln!("note: snippet has {count} fragments; printed 1/{count} (use --fragment N)");
    }
    let full = fragment.content.as_str();
    let (selected, first_line) = match range {
        Some(range) => select_lines(full, range)?,
        None => (full, 1),
    };
    let shown = match args.max_bytes {
        Some(max) => cut(selected, max),
        None => selected,
    };
    if shown.len() < full.len() {
        let total_lines = full.lines().count();
        let printed = shown.lines().count().max(usize::from(!shown.is_empty()));
        let last_line = (first_line + printed).saturating_sub(1).max(first_line);
        eprintln!(
            "note: printed lines {first_line}-{last_line} of {total_lines} ({} of {})",
            human_bytes(shown.len()),
            human_bytes(full.len())
        );
    }
    print!("{shown}");
    io::stdout().flush()?;
    Ok(())
}

pub fn command_preview(
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

pub fn resolve_managed_path(
    library: &Library,
    selector: &str,
    metadata: bool,
    readme: bool,
    fragment: Option<&str>,
) -> Result<PathBuf> {
    let catalog = library.scan()?;
    let snippet = library.resolve_snippet(&catalog, selector)?;
    Ok(if metadata {
        snippet.package_path.join("snippet.toml")
    } else if readme {
        snippet.package_path.join("README.md")
    } else if let Some(fragment) = fragment {
        library
            .resolve_fragment(snippet, Some(fragment))?
            .absolute_path
            .clone()
    } else {
        snippet.package_path.clone()
    })
}

pub fn command_path(library: &Library, args: &PathArgs) -> Result<()> {
    let path = resolve_managed_path(
        library,
        &args.selector,
        args.metadata,
        args.readme,
        args.fragment.as_deref(),
    )?;
    println!("{}", path.display());
    Ok(())
}

pub fn command_open(
    library: &Library,
    args: &OpenArgs,
    output: OutputMode,
    config: &AppConfig,
) -> Result<()> {
    let path = resolve_managed_path(
        library,
        &args.selector,
        args.metadata,
        args.readme,
        args.fragment.as_deref(),
    )?;
    let app = args
        .app
        .as_deref()
        .or(config.gui_editor_command())
        .unwrap_or("code");
    let parts = shlex::split(app)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(|| SnipError::usage(format!("invalid app command: {app:?}")))?;
    ProcessCommand::new(&parts[0])
        .args(&parts[1..])
        .arg(&path)
        .spawn()
        .map_err(|error| SnipError::io(format!("cannot launch {:?}: {error}", parts[0])))?;
    if output == OutputMode::Human {
        println!("opened: {}", path.display());
    } else {
        print_record(&json!({"opened": path, "app": parts[0]}), output)?;
    }
    Ok(())
}

fn filter_snippets<'a>(
    catalog: &'a CatalogSnapshot,
    folder: Option<FolderFilter<'_>>,
    tag: Option<&str>,
) -> Vec<&'a Snippet> {
    catalog
        .snippets
        .iter()
        .filter(|snippet| {
            folder.is_none_or(|folder| folder.matches(&snippet.folder))
                && tag.is_none_or(|tag| {
                    snippet
                        .tags
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(tag))
                })
        })
        .collect()
}

fn snippet_brief(snippet: &&Snippet) -> serde_json::Value {
    json!({
        "id": snippet.id,
        "title": snippet.title,
        "folder": snippet.folder,
        "tags": snippet.tags,
        "fragments": snippet.loaded_fragments.len(),
        "bytes": snippet.content_bytes(),
        "fingerprint": snippet.fingerprint.short(),
        "modified_at": snippet.modified_at,
    })
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
