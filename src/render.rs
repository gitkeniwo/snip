use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use serde::{Deserialize, Serialize};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::{LinesWithEndings, as_24_bit_terminal_escaped};

use crate::domain::Snippet;
use crate::error::{Result, SnipError};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    Ansi,
    Plain,
    Html,
}

pub fn preview(snippet: &Snippet, mode: RenderMode, color: bool) -> Result<String> {
    match mode {
        RenderMode::Plain => Ok(render_plain(snippet)),
        RenderMode::Ansi if !color => Ok(render_plain(snippet)),
        RenderMode::Ansi => render_ansi(snippet),
        RenderMode::Html => render_html(snippet),
    }
}

fn render_plain(snippet: &Snippet) -> String {
    let mut output = String::new();
    output.push_str(&snippet.title);
    output.push('\n');
    output.push_str(&"=".repeat(snippet.title.chars().count().max(1)));
    output.push('\n');
    if !snippet.folder.is_empty() {
        output.push_str(&format!("Folder: {}\n", snippet.folder));
    }
    if !snippet.tags.is_empty() {
        output.push_str(&format!("Tags: {}\n", snippet.tags.join(", ")));
    }
    if let Some(readme) = &snippet.readme {
        output.push('\n');
        output.push_str(&markdown_to_text(readme));
        ensure_newline(&mut output);
    }
    for (index, fragment) in snippet.loaded_fragments.iter().enumerate() {
        output.push_str(&format!(
            "\n--- {}. {} ({}) ---\n",
            index + 1,
            fragment.title,
            fragment.language
        ));
        if let Some(note) = &fragment.note_content {
            output.push_str(&markdown_to_text(note));
            ensure_newline(&mut output);
            output.push('\n');
        }
        output.push_str(&fragment.content);
        ensure_newline(&mut output);
    }
    output
}

fn render_ansi(snippet: &Snippet) -> Result<String> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let theme = select_theme(&theme_set)?;
    let mut output = format!("\x1b[1;36m{}\x1b[0m\n", snippet.title);
    if !snippet.folder.is_empty() {
        output.push_str(&format!("\x1b[2mFolder:\x1b[0m {}\n", snippet.folder));
    }
    if !snippet.tags.is_empty() {
        output.push_str(&format!(
            "\x1b[2mTags:\x1b[0m \x1b[33m{}\x1b[0m\n",
            snippet.tags.join(", ")
        ));
    }
    if let Some(readme) = &snippet.readme {
        output.push('\n');
        output.push_str(&markdown_to_ansi(readme));
        ensure_newline(&mut output);
    }
    for (index, fragment) in snippet.loaded_fragments.iter().enumerate() {
        output.push_str(&format!(
            "\n\x1b[1;35m--- {}. {}\x1b[0m \x1b[2m({})\x1b[0m\n",
            index + 1,
            fragment.title,
            fragment.language
        ));
        if let Some(note) = &fragment.note_content {
            output.push_str(&markdown_to_ansi(note));
            ensure_newline(&mut output);
            output.push('\n');
        }
        let syntax = find_syntax(&syntax_set, &fragment.language, &fragment.file);
        let mut highlighter = HighlightLines::new(syntax, theme);
        for line in LinesWithEndings::from(&fragment.content) {
            let ranges = highlighter
                .highlight_line(line, &syntax_set)
                .map_err(|error| {
                    SnipError::validation(format!("cannot highlight {}: {error}", fragment.title))
                })?;
            output.push_str(&as_24_bit_terminal_escaped(&ranges, false));
        }
        output.push_str("\x1b[0m");
        ensure_newline(&mut output);
    }
    Ok(output)
}

fn render_html(snippet: &Snippet) -> Result<String> {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let theme = select_theme(&theme_set)?;
    let mut output = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>body{font-family:-apple-system,BlinkMacSystemFont,sans-serif;max-width:1000px;margin:2rem auto;padding:0 1rem}pre{overflow:auto;padding:1rem;border-radius:.5rem;background:#f5f5f5}code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace}.meta{color:#666}.fragment{margin-top:2rem}</style></head><body>",
    );
    output.push_str(&format!("<h1>{}</h1>", escape_html(&snippet.title)));
    output.push_str(&format!(
        "<p class=\"meta\">Folder: {} · Tags: {}</p>",
        escape_html(if snippet.folder.is_empty() {
            "Uncategorized"
        } else {
            &snippet.folder
        }),
        escape_html(&snippet.tags.join(", "))
    ));
    if let Some(readme) = &snippet.readme {
        push_markdown_html(&mut output, readme);
    }
    for (index, fragment) in snippet.loaded_fragments.iter().enumerate() {
        output.push_str(&format!(
            "<section class=\"fragment\"><h2>{}. {} <small>({})</small></h2>",
            index + 1,
            escape_html(&fragment.title),
            escape_html(&fragment.language)
        ));
        if let Some(note) = &fragment.note_content {
            push_markdown_html(&mut output, note);
        }
        let syntax = find_syntax(&syntax_set, &fragment.language, &fragment.file);
        let highlighted = syntect::html::highlighted_html_for_string(
            &fragment.content,
            &syntax_set,
            syntax,
            theme,
        )
        .map_err(|error| {
            SnipError::validation(format!("cannot render {} as HTML: {error}", fragment.title))
        })?;
        output.push_str(&highlighted);
        output.push_str("</section>");
    }
    output.push_str("</body></html>\n");
    Ok(output)
}

/// Resolve a language exactly the same way for CLI and interactive previews.
pub fn find_syntax<'a>(
    syntax_set: &'a SyntaxSet,
    language: &str,
    file: &str,
) -> &'a SyntaxReference {
    syntax_set
        .find_syntax_by_token(language)
        .or_else(|| {
            std::path::Path::new(file)
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(|extension| syntax_set.find_syntax_by_extension(extension))
        })
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
}

fn push_markdown_html(output: &mut String, markdown: &str) {
    html::push_html(output, Parser::new_ext(markdown, markdown_options()));
}

fn markdown_to_text(markdown: &str) -> String {
    let mut output = String::new();
    for event in Parser::new_ext(markdown, markdown_options()) {
        match event {
            Event::Text(value) | Event::Code(value) => output.push_str(&value),
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item) => {
                ensure_newline(&mut output)
            }
            Event::Start(Tag::Item) => output.push_str("- "),
            Event::Rule => output.push_str("---\n"),
            _ => {}
        }
    }
    output
}

fn markdown_to_ansi(markdown: &str) -> String {
    let mut output = String::new();
    for event in Parser::new_ext(markdown, markdown_options()) {
        match event {
            Event::Start(Tag::Strong) => output.push_str("\x1b[1m"),
            Event::End(TagEnd::Strong) => output.push_str("\x1b[0m"),
            Event::Start(Tag::Emphasis) => output.push_str("\x1b[3m"),
            Event::End(TagEnd::Emphasis) => output.push_str("\x1b[0m"),
            Event::Start(Tag::Heading { .. }) => output.push_str("\x1b[1;36m"),
            Event::End(TagEnd::Heading(_)) => output.push_str("\x1b[0m\n"),
            Event::Code(value) => output.push_str(&format!("\x1b[33m{value}\x1b[0m")),
            Event::Text(value) => output.push_str(&value),
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::End(TagEnd::Paragraph | TagEnd::Item) => ensure_newline(&mut output),
            Event::Start(Tag::Item) => output.push_str("\x1b[2m•\x1b[0m "),
            Event::Rule => output.push_str("\x1b[2m────────────────\x1b[0m\n"),
            _ => {}
        }
    }
    output
}

fn select_theme(theme_set: &ThemeSet) -> Result<&Theme> {
    theme_set
        .themes
        .get("base16-ocean.dark")
        .or_else(|| theme_set.themes.values().next())
        .ok_or_else(|| SnipError::validation("syntect did not load any themes"))
}

fn ensure_newline(output: &mut String) {
    if !output.ends_with('\n') {
        output.push('\n');
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
