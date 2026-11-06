use std::sync::Arc;

use serde_json::json;
use snip::Library;
use snip::config::{AppConfig, PreviewRenderSetting};
use snip::domain::{CatalogSnapshot, FolderFilter, Snippet, folder_label};
use snip::error::{Result, SnipError};
use snip::render::{RenderMode, preview};
use snip::search::{MemoryIndex, SearchIndex, SearchQuery};
use snip::service::trash_entries;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use super::output::{display_json_scalar, print_record, print_records, send_to_pager};
use crate::cli::{
    ColorMode, FilterArgs, FragmentSelectorArgs, OpenArgs, OutputMode, PathArgs, PreviewArgs,
    RenderArg, SearchArgs, SelectorArgs,
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
        .map(|snippet| snippet_summary(snippet, &catalog))
        .collect::<Vec<_>>();
    if output == OutputMode::Human {
        for snippet in snippets {
            println!(
                "{}  {}  [{}]  {}",
                catalog.citation_prefix(snippet.id),
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
    let catalog = Arc::new(library.scan()?);
    let index = MemoryIndex::new(Arc::clone(&catalog));
    let query = SearchQuery::new(&args.query, args.regex)?
        .folder(args.folder_filter())
        .tag(args.tag.as_deref())
        .fields(&args.fields)
        .context_lines(args.context)
        .limit(args.limit);
    let results = index.search(&query);
    if output == OutputMode::Human {
        for result in results {
            let location = result
                .line
                .map(|line| format!(":{line}"))
                .unwrap_or_default();
            println!(
                "{}{}  {}  {}",
                catalog.citation_prefix(result.snippet_id),
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
        let rows = results
            .iter()
            .map(|result| -> Result<serde_json::Value> {
                let mut value = serde_json::to_value(result)?;
                value["citation"] = json!(catalog.citation_prefix(result.snippet_id));
                Ok(value)
            })
            .collect::<Result<Vec<_>>>()?;
        print_records(&rows, output)?;
    }
    Ok(())
}

/// Resolves `<snippet>[#n]` plus an optional `--fragment`, rejecting a fragment given twice.
pub(crate) fn resolve_cited<'a>(
    library: &Library,
    catalog: &'a CatalogSnapshot,
    selector: &str,
    explicit: Option<&str>,
) -> Result<(&'a Snippet, Option<String>)> {
    let (snippet, cited) = library.resolve_citation(catalog, selector)?;
    if cited.is_some() && explicit.is_some() {
        return Err(SnipError::usage(
            "fragment given twice (`#n` and `--fragment`)",
        ));
    }
    Ok((snippet, cited.or(explicit).map(str::to_owned)))
}

pub fn command_show(library: &Library, args: &SelectorArgs, output: OutputMode) -> Result<()> {
    let catalog = library.scan()?;
    let (snippet, cited) = library.resolve_citation(&catalog, &args.selector)?;
    let cited_index = match cited {
        Some(selector) => {
            let fragment = library.resolve_fragment(snippet, Some(selector))?;
            snippet
                .loaded_fragments
                .iter()
                .position(|candidate| candidate.id == fragment.id)
        }
        None => None,
    };
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
                "\n---{} {}. {} ({}) [{}] ---",
                if cited_index == Some(index) {
                    " ▸"
                } else {
                    ""
                },
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
        let mut value = serde_json::to_value(snippet)?;
        value["citation"] = json!(catalog.citation_prefix(snippet.id));
        if let Some(index) = cited_index {
            value["cited_fragment"] = json!(index + 1);
        }
        print_record(&value, output)?;
    }
    Ok(())
}

pub fn command_cat(library: &Library, args: &FragmentSelectorArgs) -> Result<()> {
    let catalog = library.scan()?;
    let (snippet, selected) =
        resolve_cited(library, &catalog, &args.selector, args.fragment.as_deref())?;
    let fragment = library.resolve_fragment(snippet, selected.as_deref())?;
    if selected.is_none() && snippet.loaded_fragments.len() > 1 {
        let count = snippet.loaded_fragments.len();
        eprintln!("note: snippet has {count} fragments; printed 1/{count} (use --fragment N)");
    }
    print!("{}", fragment.content);
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
    let (snippet, cited) = library.resolve_citation(&catalog, selector)?;
    if cited.is_some() && (metadata || readme) {
        return Err(SnipError::usage(
            "`#n` names a fragment; it cannot be combined with --readme/--metadata",
        ));
    }
    if cited.is_some() && fragment.is_some() {
        return Err(SnipError::usage(
            "fragment given twice (`#n` and `--fragment`)",
        ));
    }
    let fragment = cited.or(fragment);
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

fn snippet_summary(snippet: &&Snippet, catalog: &CatalogSnapshot) -> serde_json::Value {
    json!({
        "id": snippet.id,
        "citation": catalog.citation_prefix(snippet.id),
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
