use super::selection::text_width;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use super::app::App;
use super::icons::snippet_badge;
use super::widgets;
use crate::config::TuiDensitySetting;

/// Minimum display cells the title keeps before decoration gives way.
const MIN_TITLE_WIDTH: usize = 10;
const BADGE_COLUMN_WIDTH: usize = 4;
const DATE_COLUMN_WIDTH: usize = 7;
const GIST_COLUMN_WIDTH: usize = 3;
const COMPACT_PIN_WIDTH: usize = 2;
const LOCKED_MARKER_WIDTH: usize = 2;

/// Optional row columns, removed in priority order to protect the title.
struct RowColumns {
    date: bool,
    gist: bool,
    badge: bool,
}

/// Chooses decoration by its measured display width rather than a terminal-width
/// threshold. The date gives way first, then the gist column, then the language
/// badge; `reserved` is state that must always remain visible.
fn row_columns(
    width: usize,
    date_available: bool,
    gist_available: bool,
    reserved: usize,
) -> RowColumns {
    let mut columns = RowColumns {
        date: date_available,
        gist: gist_available,
        badge: true,
    };
    let fits = |columns: &RowColumns| {
        reserved
            + MIN_TITLE_WIDTH
            + usize::from(columns.date) * DATE_COLUMN_WIDTH
            + usize::from(columns.gist) * GIST_COLUMN_WIDTH
            + usize::from(columns.badge) * BADGE_COLUMN_WIDTH
            <= width
    };
    if !fits(&columns) {
        columns.date = false;
    }
    if !fits(&columns) {
        columns.gist = false;
    }
    if !fits(&columns) {
        columns.badge = false;
    }
    columns
}

fn compact_gist_column<I>(width: usize, any_badge: bool, rows: I) -> bool
where
    I: IntoIterator<Item = (bool, bool)>,
{
    any_badge
        && rows.into_iter().all(|(date_available, locked)| {
            row_columns(
                width,
                date_available,
                true,
                COMPACT_PIN_WIDTH + usize::from(locked) * LOCKED_MARKER_WIDTH,
            )
            .gist
        })
}

/// Width of the gist marker in a row: one leading space plus the glyph, or
/// zero when the snippet has no badge. Rendered immediately before the locked
/// marker, so both reserve the same space in the row arithmetic.
fn gist_marker(app: &App, snippet: &crate::domain::Snippet) -> Option<[Span<'static>; 2]> {
    app.gist_badges
        .get(&snippet.id)
        .map(|badge| crate::tui::gist_panel::glyph(*badge, app.theme))
}

/// Comfortable rows use two terminal lines; compact rows use one. Mouse
/// hit-testing derives the row height from the same density setting.
pub fn items(app: &App, width: u16) -> Vec<ListItem<'static>> {
    let width = width as usize;
    // Compact rows reserve the badge cell for every row so the date column stays
    // aligned, but only when some visible snippet is actually published —
    // a library with no gists gives up no width at all.
    let any_badge = app
        .visible
        .iter()
        .any(|row| app.gist_badges.contains_key(&row.snippet_id));
    // Gist padding keeps the date aligned down the compact list, so it must be
    // an all-or-nothing table decision rather than something each row chooses.
    let show_compact_gist = compact_gist_column(
        width,
        any_badge,
        app.visible.iter().filter_map(|row| {
            app.catalog
                .snippets
                .iter()
                .find(|snippet| snippet.id == row.snippet_id)
                .map(|snippet| (snippet_date_yymmdd(snippet).is_some(), snippet.locked))
        }),
    );
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
            if app.density == TuiDensitySetting::Compact {
                let columns = row_columns(
                    width,
                    date_str.is_some(),
                    show_compact_gist,
                    COMPACT_PIN_WIDTH + usize::from(snippet.locked) * LOCKED_MARKER_WIDTH,
                );
                let line = compact_line(
                    app,
                    snippet,
                    width,
                    date_str.filter(|_| columns.date),
                    columns.gist,
                    columns.badge,
                );
                let line = if app.focus != super::state::Pane::List
                    && app.list_state.selected() == Some(index)
                {
                    retained_line(line, app.theme)
                } else {
                    line
                };
                return Some(ListItem::new(line));
            }
            // Comfortable rows carry the badge at the end of line two, so line
            // one reserves nothing for it.
            let badge = gist_marker(app, snippet);
            let badge_width = badge
                .as_ref()
                .map_or(0, |_| crate::tui::gist_panel::GLYPH_WIDTH + 1);
            let marker_width = usize::from(snippet.locked) * LOCKED_MARKER_WIDTH;
            let columns = row_columns(width, date_str.is_some(), false, marker_width);
            let left_width = usize::from(columns.badge) * BADGE_COLUMN_WIDTH;
            let date_width = usize::from(columns.date) * DATE_COLUMN_WIDTH;
            let title_width = width.saturating_sub(left_width + date_width + marker_width);
            let title = widgets::truncate_end(&snippet.title, title_width);
            let used = left_width + text_width(&title) as usize + date_width + marker_width;
            let padding = " ".repeat(width.saturating_sub(used));
            let mut first = Vec::new();
            if columns.badge {
                first.extend([
                    Span::styled(
                        format!("{:<3}", snippet_badge(snippet)),
                        Style::default().fg(app.theme.accent_alt),
                    ),
                    Span::raw(" "),
                ]);
            }
            first.extend([
                Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(padding),
            ]);
            if let Some(date) = date_str.filter(|_| columns.date) {
                first.push(Span::styled(
                    format!(" {date}"),
                    Style::default().fg(app.theme.muted),
                ));
            }
            if snippet.locked {
                first.push(Span::styled(" ⊘", Style::default().fg(app.theme.error)));
            }

            // The second line is built into a narrowed width so the badge has
            // somewhere to sit, then right-aligned into the tail it left free.
            let second_width = width.saturating_sub(badge_width);
            let mut second = if let Some(excerpt) = row.excerpt.as_ref() {
                let indent = 3.min(second_width);
                Line::from(vec![
                    pin_gutter(app, snippet.pinned, indent),
                    Span::styled(
                        widgets::truncate_end(excerpt, second_width.saturating_sub(indent)),
                        Style::default().fg(app.theme.muted),
                    ),
                ])
            } else {
                metadata_line(app, snippet, second_width)
            };
            if let Some(spans) = badge {
                let pad =
                    width.saturating_sub(second.width() + crate::tui::gist_panel::GLYPH_WIDTH);
                second.spans.push(Span::raw(" ".repeat(pad)));
                second.spans.extend(spans);
            }
            let first = Line::from(first);
            let first = if app.focus != super::state::Pane::List
                && app.list_state.selected() == Some(index)
            {
                retained_line(first, app.theme)
            } else {
                first
            };
            Some(ListItem::new(vec![first, second]))
        })
        .collect()
}

fn retained_line(mut line: Line<'static>, theme: super::theme::TuiTheme) -> Line<'static> {
    for span in &mut line.spans {
        let preferred = span.style.fg.unwrap_or(theme.accent);
        span.style = span
            .style
            .fg(theme.legible_on(theme.retained_bg, preferred))
            .bg(theme.retained_bg)
            .add_modifier(Modifier::BOLD);
    }
    line
}

fn compact_line(
    app: &App,
    snippet: &crate::domain::Snippet,
    width: usize,
    date: Option<String>,
    show_gist: bool,
    show_badge: bool,
) -> Line<'static> {
    let badge_width = usize::from(show_badge) * BADGE_COLUMN_WIDTH;
    let date_width = usize::from(date.is_some()) * DATE_COLUMN_WIDTH;
    let gist_width = usize::from(show_gist) * GIST_COLUMN_WIDTH;
    let marker_width = usize::from(snippet.locked) * LOCKED_MARKER_WIDTH;
    let available = width
        .saturating_sub(badge_width + COMPACT_PIN_WIDTH + date_width + gist_width + marker_width);
    let folder = format!(
        " [{}]",
        crate::domain::folder_label(&snippet.folder).replace('/', " > ")
    );
    let tags = snippet
        .tags
        .iter()
        .map(|tag| format!(" #{tag}"))
        .collect::<String>();
    let (title, folder, tags) = compact_fields(&snippet.title, &folder, &tags, available);
    let used = badge_width
        + COMPACT_PIN_WIDTH
        + text_width(&title) as usize
        + text_width(&folder) as usize
        + text_width(&tags) as usize
        + date_width
        + gist_width
        + marker_width;

    let mut spans = Vec::new();
    if show_badge {
        spans.extend([
            Span::styled(
                format!("{:<3}", snippet_badge(snippet)),
                Style::default().fg(app.theme.accent_alt),
            ),
            Span::raw(" "),
        ]);
    }
    spans.extend([
        compact_pin(snippet.pinned, app.theme.warning),
        Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(folder, Style::default().fg(app.theme.muted)),
        Span::styled(tags, Style::default().fg(app.theme.muted)),
        Span::raw(" ".repeat(width.saturating_sub(used))),
    ]);
    // Ahead of the date, and padded to a constant width when any row carries a
    // badge, so the six-digit date column lines up down the whole list.
    if show_gist {
        match gist_marker(app, snippet) {
            Some(badge) => {
                spans.push(Span::raw(" "));
                spans.extend(badge);
            }
            None => spans.push(Span::raw("   ")),
        }
    }
    if let Some(date) = date {
        spans.push(Span::styled(
            format!(" {date}"),
            Style::default().fg(app.theme.muted),
        ));
    }
    if snippet.locked {
        spans.push(Span::styled(" ⊘", Style::default().fg(app.theme.error)));
    }
    Line::from(spans)
}

fn compact_fields(
    title: &str,
    folder: &str,
    tags: &str,
    available: usize,
) -> (String, String, String) {
    // Compact mode spends width in semantic priority order. A long title may
    // intentionally consume the row before folder and tag metadata.
    let title = widgets::truncate_end(title, available);
    let remaining = available.saturating_sub(text_width(&title) as usize);
    let folder = widgets::truncate_end(folder, remaining);
    let remaining = remaining.saturating_sub(text_width(&folder) as usize);
    let tags = widgets::truncate_end(tags, remaining);
    (title, folder, tags)
}

fn compact_pin(pinned: bool, color: ratatui::style::Color) -> Span<'static> {
    if pinned {
        Span::styled("★ ", Style::default().fg(color))
    } else {
        Span::raw("  ")
    }
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
    // The three-cell badge plus its separator occupy the first four cells of
    // row one. Indenting metadata by the same amount aligns it with the title.
    let indent = 4.min(width);
    let folder = widgets::truncate_end(&folder, width.saturating_sub(indent));
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
        Span::styled(
            format!(" ★ {}", " ".repeat(width - 3)),
            Style::default().fg(app.theme.warning),
        )
    } else {
        Span::raw(" ".repeat(width))
    }
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
                remotes: vec![],
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

        for width in [30, 20, 14] {
            let columns = row_columns(width, true, false, 0);
            let left_width = usize::from(columns.badge) * BADGE_COLUMN_WIDTH;
            let date_width = usize::from(columns.date) * DATE_COLUMN_WIDTH;
            let title = widgets::truncate_end(
                &snippet.manifest.title,
                width.saturating_sub(left_width + date_width),
            );
            let title_w = text_width(&title) as usize;
            let used = left_width + title_w + date_width;
            let padding_len = width.saturating_sub(used);
            assert_eq!(left_width + title_w + padding_len + date_width, width);
            assert!(title_w >= MIN_TITLE_WIDTH, "width: {width}");
        }
    }

    #[test]
    fn row_columns_protect_the_title_in_priority_order() {
        let full = row_columns(21, true, false, 0);
        assert!(full.date);
        assert!(full.badge);

        let without_date = row_columns(20, true, false, 0);
        assert!(!without_date.date);
        assert!(without_date.badge);

        let without_badge = row_columns(13, true, false, 0);
        assert!(!without_badge.date);
        assert!(!without_badge.badge);

        let compact_full = row_columns(26, true, true, COMPACT_PIN_WIDTH);
        assert!(compact_full.date);
        assert!(compact_full.gist);
        assert!(compact_full.badge);

        let compact_without_date = row_columns(25, true, true, COMPACT_PIN_WIDTH);
        assert!(!compact_without_date.date);
        assert!(compact_without_date.gist);
        assert!(compact_without_date.badge);

        let compact_without_gist = row_columns(18, true, true, COMPACT_PIN_WIDTH);
        assert!(!compact_without_gist.date);
        assert!(!compact_without_gist.gist);
        assert!(compact_without_gist.badge);
    }

    #[test]
    fn compact_gist_column_is_a_whole_list_decision() {
        assert!(compact_gist_column(20, true, [(false, false)]));
        assert!(!compact_gist_column(
            20,
            true,
            [(false, false), (true, true)]
        ));
    }

    #[test]
    fn compact_fields_spend_width_on_title_before_metadata() {
        let (title, folder, tags) =
            compact_fields("Compress Video", " [Video CLI]", " #ffmpeg", 14);
        assert_eq!(title, "Compress Video");
        assert!(folder.is_empty());
        assert!(tags.is_empty());

        let (title, folder, tags) = compact_fields("Alpha", " [AI]", " #rust", 14);
        assert_eq!(title, "Alpha");
        assert_eq!(folder, " [AI]");
        assert_eq!(tags, " #r…");
    }

    #[test]
    fn compact_pin_uses_a_fixed_visible_gutter() {
        let color = ratatui::style::Color::Yellow;
        assert_eq!(compact_pin(true, color).content.as_ref(), "★ ");
        assert_eq!(compact_pin(false, color).content.as_ref(), "  ");
    }

    #[test]
    fn compact_state_markers_survive_after_optional_columns_are_removed() {
        let temporary = tempfile::tempdir().unwrap();
        let library =
            crate::filesystem::Library::init(&temporary.path().join("Test.sniplib"), None).unwrap();
        let app = App::new(library, &crate::config::AppConfig::default()).unwrap();
        let mut snippet = make_test_snippet("2026-07-26T12:00:00Z", None);
        snippet.manifest.title = "A title that needs truncating".to_owned();
        snippet.manifest.pinned = true;
        snippet.manifest.locked = true;

        let line = compact_line(&app, &snippet, 14, None, false, false);
        let text = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(text.contains("★ "));
        assert!(text.contains(" ⊘"));
        assert_eq!(text_width(&text), 14);
    }
}
