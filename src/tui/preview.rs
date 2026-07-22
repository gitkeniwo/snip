use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::domain::Snippet;
use crate::error::Result;

use super::highlight::Highlighter;
use super::icons::snippet_badge;
use super::theme::TuiTheme;

#[derive(Default)]
pub struct PreviewCache {
    key: Option<(String, usize)>,
    text: Option<Text<'static>>,
}

impl PreviewCache {
    pub fn invalidate(&mut self) {
        self.key = None;
        self.text = None;
    }

    pub fn get(
        &mut self,
        snippet: &Snippet,
        fragment_index: usize,
        highlighter: &Highlighter,
        theme: TuiTheme,
    ) -> Result<Text<'static>> {
        let key = (snippet.fingerprint.0.clone(), fragment_index);
        if self.key.as_ref() == Some(&key) {
            return Ok(self.text.clone().unwrap_or_default());
        }
        let text = build(snippet, fragment_index, highlighter, theme)?;
        self.key = Some(key);
        self.text = Some(text.clone());
        Ok(text)
    }
}

fn build(
    snippet: &Snippet,
    fragment_index: usize,
    highlighter: &Highlighter,
    theme: TuiTheme,
) -> Result<Text<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("[{}] ", snippet_badge(snippet)),
            Style::default().fg(theme.accent_alt),
        ),
        Span::styled(
            snippet.title.clone(),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            snippet.fingerprint.0.chars().take(8).collect::<String>(),
            Style::default().fg(theme.muted),
        ),
    ])];
    let folder = if snippet.folder.is_empty() {
        "Uncategorized"
    } else {
        &snippet.folder
    };
    lines.push(Line::from(vec![
        Span::styled("Folder: ", Style::default().fg(theme.muted)),
        Span::raw(folder.to_owned()),
    ]));
    if !snippet.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Tags: ", Style::default().fg(theme.muted)),
            Span::styled(snippet.tags.join(", "), Style::default().fg(theme.warning)),
        ]));
    }
    if let Some(readme) = &snippet.readme {
        lines.push(Line::default());
        lines.extend(highlighter.markdown(readme, theme).lines);
    }
    if let Some(fragment) = snippet.loaded_fragments.get(fragment_index) {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled(
                fragment.title.clone(),
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({})", fragment.language),
                Style::default().fg(theme.muted),
            ),
        ]));
        if let Some(note) = &fragment.note_content {
            lines.extend(highlighter.markdown(note, theme).lines);
            lines.push(Line::default());
        }
        lines.extend(highlighter.fragment(fragment)?.lines);
    }
    Ok(Text::from(lines))
}
