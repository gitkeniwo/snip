use super::selection::text_width;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;
use regex::{Regex, RegexBuilder};

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

struct TableColumns {
    gist: bool,
    badge: bool,
    pin_gutter: bool,
}

/// Chooses decoration by its measured display width rather than a terminal-width
/// threshold. The date gives way first, then the gist column, then the language
/// badge; `reserved` is state that must always remain visible.
fn row_columns(
    width: usize,
    date_available: bool,
    gist_available: bool,
    badge_available: bool,
    reserved: usize,
) -> RowColumns {
    let mut columns = RowColumns {
        date: date_available,
        gist: gist_available,
        badge: badge_available,
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

/// Optional gutters that shift aligned row content use one conservative
/// decision for the whole table. Dates remain per-row because they are
/// right-aligned.
fn table_columns<I>(width: usize, gist_available: bool, compact: bool, rows: I) -> TableColumns
where
    I: IntoIterator<Item = (bool, bool, bool)>,
{
    let rows = rows.into_iter().collect::<Vec<_>>();
    let fixed_width = |locked| {
        usize::from(compact) * COMPACT_PIN_WIDTH + usize::from(locked) * LOCKED_MARKER_WIDTH
    };
    let gist = compact
        && gist_available
        && rows.iter().all(|&(date_available, locked, _)| {
            row_columns(width, date_available, true, true, fixed_width(locked)).gist
        });
    let badge = rows.iter().all(|&(date_available, locked, _)| {
        row_columns(width, date_available, gist, true, fixed_width(locked)).badge
    });
    let pin_gutter = rows.iter().any(|&(_, _, pinned)| pinned);
    TableColumns {
        gist,
        badge,
        pin_gutter,
    }
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
    let search_matcher = literal_search_matcher(&app.search.query);
    // Compact rows reserve the badge cell for every row so the date column stays
    // aligned, but only when some visible snippet is actually published —
    // a library with no gists gives up no width at all.
    let any_badge = app
        .visible
        .iter()
        .any(|row| app.gist_badges.contains_key(&row.snippet_id));
    // Badge and gist columns shift every title, while the pin gutter shifts the
    // second line. All three are table decisions so mixed state stays aligned.
    let table_columns = table_columns(
        width,
        any_badge,
        app.density == TuiDensitySetting::Compact,
        app.visible.iter().filter_map(|row| {
            app.catalog
                .snippets
                .iter()
                .find(|snippet| snippet.id == row.snippet_id)
                .map(|snippet| {
                    (
                        snippet_date_yymmdd(snippet).is_some(),
                        snippet.locked,
                        snippet.pinned,
                    )
                })
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
                    table_columns.gist,
                    table_columns.badge,
                    COMPACT_PIN_WIDTH + usize::from(snippet.locked) * LOCKED_MARKER_WIDTH,
                );
                let line = compact_line(
                    app,
                    snippet,
                    width,
                    date_str.filter(|_| columns.date),
                    columns.gist,
                    columns.badge,
                    search_matcher.as_ref(),
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
            let columns = row_columns(
                width,
                date_str.is_some(),
                false,
                table_columns.badge,
                marker_width,
            );
            let left_width = usize::from(columns.badge) * BADGE_COLUMN_WIDTH;
            let date_width = usize::from(columns.date) * DATE_COLUMN_WIDTH;
            let title_width = width.saturating_sub(left_width + date_width + marker_width);
            let title = truncate_end_guarded(
                &snippet.title,
                title_width,
                date_width == 0 && marker_width == 0,
            );
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
            first.extend(search_spans(
                app,
                search_matcher.as_ref(),
                &title,
                Style::default().add_modifier(Modifier::BOLD),
            ));
            first.push(Span::raw(padding));
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
            let indent = second_line_indent(columns.badge, table_columns.pin_gutter, second_width);
            let mut second = if let Some(excerpt) = row.excerpt.as_ref() {
                let excerpt = truncate_search_excerpt(
                    excerpt,
                    second_width.saturating_sub(indent),
                    badge.is_none(),
                    search_matcher.as_ref(),
                );
                let mut spans = vec![pin_gutter(app, snippet.pinned, indent)];
                spans.extend(search_spans(
                    app,
                    search_matcher.as_ref(),
                    &excerpt,
                    Style::default().fg(app.theme.muted),
                ));
                Line::from(spans)
            } else {
                metadata_line(
                    app,
                    snippet,
                    second_width,
                    indent,
                    badge.is_none(),
                    search_matcher.as_ref(),
                )
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
    search_matcher: Option<&Regex>,
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
    let (title, folder, tags) = compact_fields(
        &snippet.title,
        &folder,
        &tags,
        available,
        date.is_none() && !show_gist && !snippet.locked,
    );
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
    spans.push(compact_pin(snippet.pinned, app.theme.warning));
    spans.extend(search_spans(
        app,
        search_matcher,
        &title,
        Style::default().add_modifier(Modifier::BOLD),
    ));
    spans.extend(search_spans(
        app,
        search_matcher,
        &folder,
        Style::default().fg(app.theme.muted),
    ));
    spans.extend(search_spans(
        app,
        search_matcher,
        &tags,
        Style::default().fg(app.theme.muted),
    ));
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
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
    guard_trailing_ellipsis: bool,
) -> (String, String, String) {
    // Compact mode spends width in semantic priority order. A long title may
    // intentionally consume the row before folder and tag metadata.
    let available = if guard_trailing_ellipsis
        && text_width(title) as usize + text_width(folder) as usize + text_width(tags) as usize
            > available
    {
        available.saturating_sub(1)
    } else {
        available
    };
    let title = widgets::truncate_end(title, available);
    let remaining = available.saturating_sub(text_width(&title) as usize);
    let folder = widgets::truncate_end(folder, remaining);
    let remaining = remaining.saturating_sub(text_width(&folder) as usize);
    let tags = widgets::truncate_end(tags, remaining);
    (title, folder, tags)
}

fn compact_pin(pinned: bool, color: ratatui::style::Color) -> Span<'static> {
    if pinned {
        Span::styled(
            "★ ",
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("  ")
    }
}

/// Compiles the literal, case-insensitive matcher once per rendered list.
fn literal_search_matcher(query: &str) -> Option<Regex> {
    if query.is_empty() {
        return None;
    }
    RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()
        .ok()
}

/// Splits visible search matches into their own spans. The warning background
/// makes matches obvious on ordinary rows; underlining survives the list's
/// selected-row style, which intentionally owns foreground and background.
fn search_spans(
    app: &App,
    matcher: Option<&Regex>,
    value: &str,
    base: Style,
) -> Vec<Span<'static>> {
    let Some(matcher) = matcher else {
        return vec![Span::styled(value.to_owned(), base)];
    };
    if value.is_empty() {
        return vec![Span::styled(value.to_owned(), base)];
    }
    let matches = matcher.find_iter(value).collect::<Vec<_>>();
    if matches.is_empty() {
        return vec![Span::styled(value.to_owned(), base)];
    }

    let matched = base
        .fg(app
            .theme
            .legible_on(app.theme.warning, app.theme.selection_fg))
        .bg(app.theme.warning)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let mut spans = Vec::with_capacity(matches.len() * 2 + 1);
    let mut start = 0;
    for found in matches {
        if start < found.start() {
            spans.push(Span::styled(value[start..found.start()].to_owned(), base));
        }
        spans.push(Span::styled(
            value[found.start()..found.end()].to_owned(),
            matched,
        ));
        start = found.end();
    }
    if start < value.len() {
        spans.push(Span::styled(value[start..].to_owned(), base));
    }
    spans
}

/// Truncates a matching line from the left when its first match would otherwise
/// fall outside the pane. Search results should always show why they matched.
fn truncate_search_excerpt(
    value: &str,
    width: usize,
    guard_trailing_ellipsis: bool,
    matcher: Option<&Regex>,
) -> String {
    if text_width(value) as usize <= width {
        return value.to_owned();
    }
    let width = if guard_trailing_ellipsis {
        width.saturating_sub(1)
    } else {
        width
    };
    let Some(found) = matcher.and_then(|matcher| matcher.find(value)) else {
        return widgets::truncate_end(value, width);
    };
    if text_width(&value[..found.end()]) as usize <= width {
        return widgets::truncate_end(value, width);
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    format!(
        "…{}",
        widgets::truncate_end(&value[found.start()..], width - 1)
    )
}

/// When an ellipsis would be the last visible cell in a pane, keep one blank
/// cell after it. Some terminal fonts draw the glyph slightly outside its cell,
/// which otherwise makes it appear to overwrite the right border.
fn truncate_end_guarded(value: &str, width: usize, guard_trailing_ellipsis: bool) -> String {
    let truncated = text_width(value) as usize > width;
    let width = if truncated && guard_trailing_ellipsis {
        width.saturating_sub(1)
    } else {
        width
    };
    widgets::truncate_end(value, width)
}

fn second_line_indent(show_badge: bool, reserve_pin_gutter: bool, width: usize) -> usize {
    (usize::from(show_badge) * BADGE_COLUMN_WIDTH)
        .max(usize::from(reserve_pin_gutter) * COMPACT_PIN_WIDTH)
        .min(width)
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

fn metadata_line(
    app: &App,
    snippet: &crate::domain::Snippet,
    width: usize,
    indent: usize,
    guard_trailing_ellipsis: bool,
    search_matcher: Option<&Regex>,
) -> Line<'static> {
    let folder_path = crate::domain::folder_label(&snippet.folder).replace('/', " > ");
    let folder = format!("[{folder_path}]");
    let natural_width = indent
        + text_width(&folder) as usize
        + snippet
            .tags
            .iter()
            .enumerate()
            .map(|(index, tag)| {
                text_width(&format!("{}#{tag}", if index == 0 { " · " } else { " " })) as usize
            })
            .sum::<usize>();
    let width = if guard_trailing_ellipsis && natural_width > width {
        width.saturating_sub(1)
    } else {
        width
    };
    // `items` matches this to the table-wide badge column, while reserving two
    // cells when a pin must survive after that column is removed.
    let indent = indent.min(width);
    let folder = widgets::truncate_end(&folder, width.saturating_sub(indent));
    let mut spans = vec![pin_gutter(app, snippet.pinned, indent)];
    spans.extend(search_spans(
        app,
        search_matcher,
        &folder,
        Style::default().fg(app.theme.muted),
    ));
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
        spans.extend(search_spans(
            app,
            search_matcher,
            &text[separator_len..],
            Style::default().fg(app.theme.tag),
        ));
    }
    Line::from(spans)
}

fn pin_gutter(app: &App, pinned: bool, width: usize) -> Span<'static> {
    let pin_style = Style::default()
        .fg(app.theme.warning)
        .add_modifier(Modifier::BOLD);
    match (pinned, width) {
        (_, 0) => Span::raw(""),
        (true, 1) => Span::styled("★", pin_style),
        (true, 2) => Span::styled("★ ", pin_style),
        (true, _) => Span::styled(format!(" ★ {}", " ".repeat(width - 3)), pin_style),
        (false, _) => Span::raw(" ".repeat(width)),
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
            let columns = row_columns(width, true, false, true, 0);
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
        let full = row_columns(21, true, false, true, 0);
        assert!(full.date);
        assert!(full.badge);

        let without_date = row_columns(20, true, false, true, 0);
        assert!(!without_date.date);
        assert!(without_date.badge);

        let without_badge = row_columns(13, true, false, true, 0);
        assert!(!without_badge.date);
        assert!(!without_badge.badge);

        let compact_full = row_columns(26, true, true, true, COMPACT_PIN_WIDTH);
        assert!(compact_full.date);
        assert!(compact_full.gist);
        assert!(compact_full.badge);

        let compact_without_date = row_columns(25, true, true, true, COMPACT_PIN_WIDTH);
        assert!(!compact_without_date.date);
        assert!(compact_without_date.gist);
        assert!(compact_without_date.badge);

        let compact_without_gist = row_columns(18, true, true, true, COMPACT_PIN_WIDTH);
        assert!(!compact_without_gist.date);
        assert!(!compact_without_gist.gist);
        assert!(compact_without_gist.badge);
    }

    #[test]
    fn shifting_columns_are_whole_list_decisions() {
        let comfortable =
            table_columns(14, false, false, [(true, true, true), (true, false, false)]);
        assert!(!comfortable.badge);
        assert!(comfortable.pin_gutter);

        let compact = table_columns(16, false, true, [(true, true, true), (true, false, false)]);
        assert!(!compact.badge);

        assert!(table_columns(20, true, true, [(false, false, false)]).gist);
        assert!(!table_columns(20, true, true, [(false, false, false), (true, true, false)]).gist);
    }

    #[test]
    fn compact_fields_spend_width_on_title_before_metadata() {
        let (title, folder, tags) =
            compact_fields("Compress Video", " [Video CLI]", " #ffmpeg", 14, false);
        assert_eq!(title, "Compress Video");
        assert!(folder.is_empty());
        assert!(tags.is_empty());

        let (title, folder, tags) = compact_fields("Alpha", " [AI]", " #rust", 14, false);
        assert_eq!(title, "Alpha");
        assert_eq!(folder, " [AI]");
        assert_eq!(tags, " #r…");
    }

    #[test]
    fn compact_pin_uses_a_fixed_visible_gutter() {
        let color = ratatui::style::Color::Yellow;
        let pin = compact_pin(true, color);
        assert_eq!(pin.content.as_ref(), "★ ");
        assert!(pin.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(compact_pin(false, color).content.as_ref(), "  ");
    }

    #[test]
    fn trailing_ellipsis_keeps_one_cell_clear_of_the_border() {
        assert_eq!(truncate_end_guarded("Alpha", 4, false), "Alp…");
        assert_eq!(truncate_end_guarded("Alpha", 4, true), "Al…");

        let temporary = tempfile::tempdir().unwrap();
        let library =
            crate::filesystem::Library::init(&temporary.path().join("Test.sniplib"), None).unwrap();
        let app = App::new(library, &crate::config::AppConfig::default()).unwrap();
        let mut snippet = make_test_snippet("2026-07-26T12:00:00Z", None);
        snippet.manifest.title = "A title that needs truncating".to_owned();
        snippet.folder = "A folder that needs truncating".to_owned();

        let compact = compact_line(&app, &snippet, 14, None, false, false, None);
        let compact_text = compact
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert_eq!(text_width(&compact_text), 14);
        assert!(compact_text.ends_with(' '));

        let metadata = metadata_line(&app, &snippet, 12, 0, true, None);
        assert_eq!(metadata.width(), 11);
        assert!(metadata.spans.last().unwrap().content.ends_with('…'));
    }

    #[test]
    fn search_excerpt_keeps_a_late_match_inside_the_visible_window() {
        let matcher = literal_search_matcher("brew").unwrap();
        let excerpt = truncate_search_excerpt(
            "Reminders, not the older tool in homebrew-core",
            20,
            false,
            Some(&matcher),
        );

        assert!(excerpt.starts_with('…'));
        assert!(excerpt.contains("brew"));
        assert!(text_width(&excerpt) <= 20);
    }

    #[test]
    fn second_line_follows_the_badge_column_without_losing_a_pin() {
        assert_eq!(second_line_indent(true, false, 12), 4);
        assert_eq!(second_line_indent(false, false, 12), 0);
        assert_eq!(second_line_indent(false, true, 12), 2);

        let temporary = tempfile::tempdir().unwrap();
        let library =
            crate::filesystem::Library::init(&temporary.path().join("Test.sniplib"), None).unwrap();
        let app = App::new(library, &crate::config::AppConfig::default()).unwrap();
        assert_eq!(pin_gutter(&app, true, 2).content.as_ref(), "★ ");

        let mut snippet = make_test_snippet("2026-07-26T12:00:00Z", None);
        snippet.folder = "Code".to_owned();
        let unindented = metadata_line(&app, &snippet, 12, 0, false, None)
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(unindented.starts_with("[Code]"));

        let aligned_unpinned = metadata_line(&app, &snippet, 12, 2, false, None)
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(aligned_unpinned.starts_with("  [Code]"));

        snippet.manifest.pinned = true;
        let pinned_line = metadata_line(&app, &snippet, 12, 2, false, None);
        let pinned = pinned_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(pinned.starts_with("★ [Code]"));
        assert!(
            pinned_line.spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
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

        let line = compact_line(&app, &snippet, 14, None, false, false, None);
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
