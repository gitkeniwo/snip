use super::selection::{char_width, text_width};
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
        .enumerate()
        .filter_map(|(index, row)| {
            let snippet = app
                .catalog
                .snippets
                .iter()
                .find(|snippet| snippet.id == row.snippet_id)?;
            let date_str = snippet_date_yymmdd(snippet);
            let date_width = if date_str.is_some() && width >= 16 {
                7
            } else {
                0
            };
            let marker_width = usize::from(snippet.locked) * 2;
            let left_width = 3;
            let title_width = width.saturating_sub(left_width + date_width + marker_width);
            let title = truncate(&snippet.title, title_width);
            let used = left_width + text_width(&title) as usize + date_width + marker_width;
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
            if let Some(date) = date_str.filter(|_| date_width > 0) {
                first.push(Span::styled(
                    format!(" {date}"),
                    Style::default().fg(app.theme.muted),
                ));
            }
            if snippet.locked {
                first.push(Span::styled(" ⊘", Style::default().fg(app.theme.error)));
            }

            let second = if let Some(excerpt) = row.excerpt.as_ref() {
                let indent = 3.min(width);
                Line::from(vec![
                    pin_gutter(app, snippet.pinned, indent),
                    Span::styled(
                        truncate(excerpt, width.saturating_sub(indent)),
                        Style::default().fg(app.theme.muted),
                    ),
                ])
            } else {
                metadata_line(app, snippet, width)
            };
            let first = Line::from(first);
            let first = if app.focus != super::state::Pane::List
                && app.list_state.selected() == Some(index)
            {
                first.style(app.theme.retained_selection())
            } else {
                first
            };
            Some(ListItem::new(vec![first, second]))
        })
        .collect()
}

fn snippet_date_yymmdd(snippet: &crate::domain::Snippet) -> Option<String> {
    let timestamp = snippet
        .modified_at
        .as_deref()
        .unwrap_or_else(|| snippet.created_at.as_str());
    if timestamp.len() >= 10 {
        let bytes = timestamp.as_bytes();
        if bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[2..4].iter().all(u8::is_ascii_digit)
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[8..10].iter().all(u8::is_ascii_digit)
        {
            return Some(format!(
                "{}{}{}",
                &timestamp[2..4],
                &timestamp[5..7],
                &timestamp[8..10]
            ));
        }
    }
    None
}

fn metadata_line(app: &App, snippet: &crate::domain::Snippet, width: usize) -> Line<'static> {
    let folder_path = crate::domain::folder_label(&snippet.folder).replace('/', " > ");
    let folder = format!("[{folder_path}]");
    // Badge (two cells) plus its separator occupy the first three cells of
    // row one. Indenting metadata by the same amount aligns it with the title.
    let indent = 3.min(width);
    let folder = truncate(&folder, width.saturating_sub(indent));
    let mut spans = vec![
        pin_gutter(app, snippet.pinned, indent),
        Span::styled(folder.clone(), Style::default().fg(app.theme.muted)),
    ];
    let mut used = indent + text_width(&folder) as usize;
    for tag in &snippet.tags {
        let text = if used == indent {
            format!("#{tag}")
        } else if spans.len() == 2 {
            format!(" · #{tag}")
        } else {
            format!(" #{tag}")
        };
        let text_w = text_width(&text) as usize;
        if used + text_w > width {
            if used < width {
                spans.push(Span::styled("…", Style::default().fg(app.theme.muted)));
            }
            break;
        }
        used += text_w;
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

fn pin_gutter(app: &App, pinned: bool, width: usize) -> Span<'static> {
    if pinned && width >= 3 {
        Span::styled(" ★ ", Style::default().fg(app.theme.warning))
    } else {
        Span::raw(" ".repeat(width))
    }
}

fn truncate(value: &str, max_width: usize) -> String {
    let total_width = text_width(value) as usize;
    if total_width <= max_width {
        return value.to_owned();
    }
    if max_width == 0 {
        return String::new();
    }
    let target = max_width.saturating_sub(1);
    let mut acc = String::new();
    let mut current_width = 0;
    for c in value.chars() {
        let cw = char_width(c) as usize;
        if current_width + cw > target {
            break;
        }
        acc.push(c);
        current_width += cw;
    }
    acc.push('…');
    acc
}
#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_snippet(created_at: &str, modified_at: Option<&str>) -> crate::domain::Snippet {
        use crate::domain::{Fingerprint, Snippet, SnippetManifest};
        use uuid::Uuid;

        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::new_v4(),
                title: "Test".to_owned(),
                tags: vec![],
                pinned: false,
                locked: false,
                created_at: created_at.to_owned(),
                source: None,
                fragments: vec![],
                extra: Default::default(),
            },
            readme: None,
            folder: String::new(),
            package_path: std::path::PathBuf::new(),
            modified_at: modified_at.map(String::from),
            fingerprint: Fingerprint("abc".to_owned()),
            loaded_fragments: vec![],
        }
    }

    #[test]
    fn test_snippet_date_yymmdd_modified_at() {
        let snippet = make_test_snippet("2025-01-01T00:00:00Z", Some("2026-07-23T21:17:18Z"));
        assert_eq!(snippet_date_yymmdd(&snippet), Some("260723".to_owned()));
    }

    #[test]
    fn test_snippet_date_yymmdd_created_at_fallback() {
        let snippet = make_test_snippet("2025-11-04T12:34:56Z", None);
        assert_eq!(snippet_date_yymmdd(&snippet), Some("251104".to_owned()));
    }

    #[test]
    fn test_snippet_date_yymmdd_invalid_format() {
        let snippet = make_test_snippet("invalid-date", None);
        assert_eq!(snippet_date_yymmdd(&snippet), None);
    }

    #[test]
    fn test_snippet_date_alignment_with_cjk_title() {
        let mut snippet = make_test_snippet("2026-07-26T12:00:00Z", None);
        snippet.manifest.title = "SQL LEETCODE 题型总结".to_owned();
        let date_str = snippet_date_yymmdd(&snippet);
        assert_eq!(date_str, Some("260726".to_owned()));

        let title = truncate(&snippet.manifest.title, 20);
        let title_w = text_width(&title) as usize;
        let left_width = 3;
        let date_width = 7;
        let marker_width = 0;
        let used = left_width + title_w + date_width + marker_width;
        let width: usize = 30;
        let padding_len = width.saturating_sub(used);
        assert_eq!(left_width + title_w + padding_len + date_width, 30);
    }
}
