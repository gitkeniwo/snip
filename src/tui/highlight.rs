use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::domain::Fragment;
use crate::error::{Result, SnipError};
use crate::render::find_syntax;

use super::theme::{Appearance, TuiTheme};

pub struct Highlighter {
    syntaxes: SyntaxSet,
    theme: Theme,
}

impl Highlighter {
    pub fn new(theme: TuiTheme) -> Result<Self> {
        let syntaxes = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        let preferred = match theme.appearance {
            Appearance::Light => "InspiredGitHub",
            Appearance::Dark => "base16-ocean.dark",
        };
        let theme = themes
            .themes
            .get(preferred)
            .or_else(|| themes.themes.values().next())
            .cloned()
            .ok_or_else(|| SnipError::validation("syntect did not load any themes"))?;
        Ok(Self { syntaxes, theme })
    }

    pub fn fragment(&self, fragment: &Fragment) -> Result<Text<'static>> {
        let syntax = find_syntax(&self.syntaxes, &fragment.language, &fragment.file);
        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut lines = Vec::new();
        for source in LinesWithEndings::from(&fragment.content) {
            let ranges = highlighter
                .highlight_line(source, &self.syntaxes)
                .map_err(|error| {
                    SnipError::validation(format!("cannot highlight {}: {error}", fragment.title))
                })?;
            let spans = ranges
                .into_iter()
                .map(|(style, value)| {
                    let mut modifiers = Modifier::empty();
                    if style.font_style.contains(FontStyle::BOLD) {
                        modifiers |= Modifier::BOLD;
                    }
                    if style.font_style.contains(FontStyle::ITALIC) {
                        modifiers |= Modifier::ITALIC;
                    }
                    if style.font_style.contains(FontStyle::UNDERLINE) {
                        modifiers |= Modifier::UNDERLINED;
                    }
                    Span::styled(
                        value.trim_end_matches(['\r', '\n']).to_owned(),
                        Style::default()
                            .fg(Color::Rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ))
                            .add_modifier(modifiers),
                    )
                })
                .collect::<Vec<_>>();
            lines.push(Line::from(spans));
        }
        if lines.is_empty() {
            lines.push(Line::default());
        }
        Ok(Text::from(lines))
    }

    pub fn markdown(&self, source: &str, theme: TuiTheme) -> Text<'static> {
        markdown(source, theme)
    }
}

pub fn markdown(markdown: &str, theme: TuiTheme) -> Text<'static> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES;
    let mut lines = vec![Line::default()];
    let mut style = Style::default();
    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(Tag::Strong) => style = style.add_modifier(Modifier::BOLD),
            Event::End(TagEnd::Strong) => style = style.remove_modifier(Modifier::BOLD),
            Event::Start(Tag::Emphasis) => style = style.add_modifier(Modifier::ITALIC),
            Event::End(TagEnd::Emphasis) => style = style.remove_modifier(Modifier::ITALIC),
            Event::Start(Tag::Heading { .. }) => {
                style = style.fg(theme.accent).add_modifier(Modifier::BOLD)
            }
            Event::End(TagEnd::Heading(_)) => {
                style = Style::default();
                lines.push(Line::default());
            }
            Event::Start(Tag::Item) => {
                push_span(&mut lines, "• ", Style::default().fg(theme.muted))
            }
            Event::Text(value) => push_span(&mut lines, value.as_ref(), style),
            Event::Code(value) => push_span(
                &mut lines,
                value.as_ref(),
                Style::default().fg(theme.warning),
            ),
            Event::SoftBreak | Event::HardBreak => lines.push(Line::default()),
            Event::End(TagEnd::Paragraph | TagEnd::Item) => lines.push(Line::default()),
            Event::Rule => {
                push_span(
                    &mut lines,
                    "────────────────",
                    Style::default().fg(theme.muted),
                );
                lines.push(Line::default());
            }
            _ => {}
        }
    }
    while lines.len() > 1 && lines.last().is_some_and(|line| line.spans.is_empty()) {
        lines.pop();
    }
    Text::from(lines)
}

fn push_span(lines: &mut Vec<Line<'static>>, value: &str, style: Style) {
    let mut parts = value.split('\n').peekable();
    while let Some(part) = parts.next() {
        if !part.is_empty() {
            lines
                .last_mut()
                .expect("markdown always has a line")
                .spans
                .push(Span::styled(part.to_owned(), style));
        }
        if parts.peek().is_some() {
            lines.push(Line::default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Fragment, FragmentManifest};
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn markdown_maps_emphasis_and_rust_gets_non_default_styles() {
        let theme = TuiTheme::for_appearance(Appearance::Dark);
        let markdown = markdown("# Heading\n\n**bold** and `code`", theme);
        assert!(
            markdown
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.style != Style::default())
        );

        let fragment = Fragment {
            manifest: FragmentManifest {
                id: Uuid::new_v4(),
                title: "Rust".to_owned(),
                language: "rust".to_owned(),
                file: "fragments/main.rs".to_owned(),
                note: None,
                source_language: None,
                extra: toml::Table::new(),
            },
            content: "fn main() { let answer = 42; }\n".to_owned(),
            note_content: None,
            absolute_path: PathBuf::from("main.rs"),
        };
        let highlighted = Highlighter::new(theme)
            .unwrap()
            .fragment(&fragment)
            .unwrap();
        assert!(
            highlighted
                .lines
                .iter()
                .flat_map(|line| &line.spans)
                .any(|span| span.style != Style::default())
        );
    }
}
