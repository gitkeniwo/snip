use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use super::app::App;
use super::icons::snippet_badge;

/// Every snippet occupies exactly two terminal rows. Mouse hit-testing relies
/// on this invariant, so metadata and excerpts are always folded into row two.
pub fn items(app: &App, width: u16) -> Vec<ListItem<'static>> {
    let width = width as usize;
    app.visible
        .iter()
        .filter_map(|row| {
            let snippet = app
                .catalog
                .snippets
                .iter()
                .find(|snippet| snippet.id == row.snippet_id)?;
            let marker_width = usize::from(snippet.pinned) * 2 + usize::from(snippet.locked) * 2;
            let title_width = width.saturating_sub(3 + marker_width);
            let title = truncate(&snippet.title, title_width);
            let used = 3 + title.chars().count() + marker_width;
            let padding = " ".repeat(width.saturating_sub(used));
            let mut first = vec![
                Span::styled(
                    snippet_badge(snippet).to_owned(),
                    Style::default().fg(app.theme.accent_alt),
                ),
                Span::raw(" "),
                Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(padding),
            ];
            if snippet.pinned {
                first.push(Span::styled(" ★", Style::default().fg(app.theme.warning)));
            }
            if snippet.locked {
                first.push(Span::styled(" ⊘", Style::default().fg(app.theme.error)));
            }

            let second = if let Some(excerpt) = row.excerpt.as_ref() {
                let indent = 3.min(width);
                Line::from(vec![
                    Span::raw(" ".repeat(indent)),
                    Span::styled(
                        truncate(excerpt, width.saturating_sub(indent)),
                        Style::default().fg(app.theme.muted),
                    ),
                ])
            } else {
                metadata_line(app, snippet, width)
            };
            Some(ListItem::new(vec![Line::from(first), second]))
        })
        .collect()
}

fn metadata_line(app: &App, snippet: &crate::domain::Snippet, width: usize) -> Line<'static> {
    let folder_path = if snippet.folder.is_empty() {
        "~".to_owned()
    } else {
        snippet.folder.replace('/', " > ")
    };
    let folder = format!("[{folder_path}]");
    // Badge (two cells) plus its separator occupy the first three cells of
    // row one. Indenting metadata by the same amount aligns it with the title.
    let indent = 3.min(width);
    let folder = truncate(&folder, width.saturating_sub(indent));
    let mut spans = vec![
        Span::raw(" ".repeat(indent)),
        Span::styled(folder.clone(), Style::default().fg(app.theme.muted)),
    ];
    let mut used = indent + folder.chars().count();
    for tag in &snippet.tags {
        let text = if used == indent {
            format!("#{tag}")
        } else if spans.len() == 2 {
            format!(" · #{tag}")
        } else {
            format!(" #{tag}")
        };
        if used + text.chars().count() > width {
            if used < width {
                spans.push(Span::styled("…", Style::default().fg(app.theme.muted)));
            }
            break;
        }
        used += text.chars().count();
        let separator_len = text.find('#').unwrap_or(0);
        if separator_len > 0 {
            spans.push(Span::styled(
                text[..separator_len].to_owned(),
                Style::default().fg(app.theme.muted),
            ));
        }
        spans.push(Span::styled(
            text[separator_len..].to_owned(),
            Style::default().fg(app.theme.tag),
        ));
    }
    Line::from(spans)
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    value
        .chars()
        .take(width.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}
