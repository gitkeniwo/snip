use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::domain::{Fragment, Snippet};

use super::super::app::App;
use super::super::icons;
use super::super::selection::{SelectionKey, text_width};
use super::super::state::Pane;
use super::super::theme::TuiTheme;
use super::super::widgets;
use super::layout::{compose_preview, wrap_preview};

const FRAGMENT_LIST_MAX_ROWS: usize = 12;
const PREVIEW_MIN_CONTENT_ROWS: u16 = 3;

pub fn draw_preview(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let snippet = app.selected_snippet().cloned();
    let block = widgets::preview_block(
        app.focus == Pane::Preview,
        app.theme,
        snippet.as_ref(),
        app.fragment_index,
        area.width,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(snippet) = snippet else {
        app.layout.preview_fragments = Rect::default();
        frame.render_widget(
            Paragraph::new("No snippets match the current filter")
                .style(Style::default().fg(app.theme.muted))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };
    let has_tags = !snippet.tags.is_empty();
    let total_fragments = snippet.loaded_fragments.len();
    let expanded = app.fragments_expanded && total_fragments > 0;
    let fragment_rows = if expanded {
        let wanted = 1 + total_fragments.min(FRAGMENT_LIST_MAX_ROWS) as u16;
        let fixed = 2 + u16::from(has_tags) + 1;
        let budget = inner
            .height
            .saturating_sub(fixed)
            .saturating_sub(PREVIEW_MIN_CONTENT_ROWS)
            .max(1);
        wanted.min(budget)
    } else {
        1
    };
    let regions = if has_tags {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(fragment_rows),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner)
    } else {
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(fragment_rows),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner)
    };
    let title_area = regions[0];
    let metadata_area = regions[1];
    let fragment_area = regions[2];
    let tags_area = has_tags.then_some(regions[3]);
    let rule_area = if has_tags { regions[4] } else { regions[3] };
    let raw_content_area = if has_tags { regions[5] } else { regions[4] };
    let content_area = if app.show_line_numbers {
        raw_content_area
    } else {
        widgets::inset_left(raw_content_area, 1)
    };
    app.layout.preview_fragments = widgets::inset_left(fragment_area, 1);
    app.layout.preview_content = content_area;
    draw_preview_header(
        frame,
        app,
        &snippet,
        PreviewHeaderAreas {
            title: title_area,
            metadata: metadata_area,
            fragment: fragment_area,
            tags: tags_area,
            rule: rule_area,
        },
    );
    match app
        .preview
        .get(&snippet, app.fragment_index, &app.highlighter, app.theme)
    {
        Ok(document) => {
            let lines = compose_preview(
                document,
                app.show_line_numbers,
                app.theme,
                content_area.width.max(1),
            );
            let rendered = wrap_preview(lines, content_area.width.max(1), app.show_line_numbers);
            app.preview_selection.prepare(
                SelectionKey {
                    snippet_id: snippet.id,
                    fragment_index: app.fragment_index,
                    fingerprint: snippet.fingerprint.0.clone(),
                },
                rendered.rows,
            );
            let max_scroll = rendered
                .text
                .lines
                .len()
                .saturating_sub(content_area.height as usize)
                .min(u16::MAX as usize) as u16;
            app.preview_scroll = app.preview_scroll.min(max_scroll);
            frame.render_widget(
                Paragraph::new(rendered.text).scroll((app.preview_scroll, 0)),
                content_area,
            );
            draw_preview_selection(frame, app, content_area);
        }
        Err(error) => {
            app.preview_selection.clear();
            frame.render_widget(
                Paragraph::new(error.to_string()).style(Style::default().fg(app.theme.error)),
                content_area,
            );
        }
    }
}

struct PreviewHeaderAreas {
    title: Rect,
    metadata: Rect,
    fragment: Rect,
    tags: Option<Rect>,
    rule: Rect,
}

fn draw_preview_header(
    frame: &mut Frame<'_>,
    app: &mut App,
    snippet: &Snippet,
    areas: PreviewHeaderAreas,
) {
    let title_area = widgets::inset_left(areas.title, 1);
    let metadata_area = widgets::inset_left(areas.metadata, 1);
    let fragment_area = widgets::inset_left(areas.fragment, 1);
    let tags_area = areas.tags.map(|area| widgets::inset_left(area, 1));
    let rule_area = widgets::inset_left(areas.rule, 1);
    let marker = match (snippet.pinned, snippet.locked) {
        (true, true) => "★ pinned · ⊘ locked".to_owned(),
        (true, false) => "★ pinned".to_owned(),
        (false, true) => "⊘ locked".to_owned(),
        (false, false) => String::new(),
    };
    let gist_marker = app
        .gist_badges
        .get(&snippet.id)
        .copied()
        .map(|badge| crate::tui::gist_panel::glyph(badge, app.icon_mode, app.theme));
    let marker_width =
        marker.chars().count() + gist_marker.map_or(0, |(glyph, _)| glyph.chars().count() + 3);
    let title_width = title_area.width.saturating_sub(marker_width as u16 + 2) as usize;
    let title = widgets::truncate_end(&snippet.title, title_width);
    let padding = " ".repeat(
        title_area
            .width
            .saturating_sub(title.chars().count() as u16 + marker_width as u16 + 3)
            as usize,
    );
    let mut spans = vec![
        Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(padding),
        Span::styled(marker, Style::default().fg(app.theme.warning)),
    ];
    if let Some((glyph, color)) = gist_marker {
        spans.push(Span::raw(" · "));
        spans.push(Span::styled(glyph.to_owned(), Style::default().fg(color)));
    }
    spans.push(Span::raw("   "));
    frame.render_widget(Paragraph::new(Line::from(spans)), title_area);
    let metadata = vec![
        Span::styled(
            crate::domain::folder_label(&snippet.folder).to_owned(),
            Style::default().fg(app.theme.muted),
        ),
        Span::styled(" · ", Style::default().fg(app.theme.muted)),
        Span::styled(
            snippet.fingerprint.0.chars().take(8).collect::<String>(),
            Style::default().fg(app.theme.muted),
        ),
    ];
    frame.render_widget(Paragraph::new(Line::from(metadata)), metadata_area);

    let total_fragments = snippet.loaded_fragments.len();
    app.layout.fragment_rows.clear();
    if app.fragments_expanded && total_fragments > 0 {
        draw_fragment_tree(frame, app, snippet, fragment_area);
    } else {
        draw_fragment_line(frame, app, snippet, fragment_area);
    }

    if let Some(tags_area) = tags_area {
        let mut tags = Vec::new();
        for tag in &snippet.tags {
            if !tags.is_empty() {
                tags.push(Span::raw(" "));
            }
            tags.push(Span::styled(
                format!("#{tag}"),
                Style::default().fg(app.theme.tag),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(tags)), tags_area);
    }
    widgets::draw_rule(frame, rule_area, app.theme);
}

/// Explicit titles are kept verbatim; only filename fallbacks lose an index prefix.
fn fragment_label(fragment: &Fragment) -> String {
    if !fragment.title.trim().is_empty() {
        return fragment.title.clone();
    }
    let Some(stem) = std::path::Path::new(&fragment.file)
        .file_stem()
        .and_then(|value| value.to_str())
    else {
        return "untitled".to_owned();
    };
    let digit_count = stem.chars().take_while(char::is_ascii_digit).count();
    let label = stem
        .get(digit_count..)
        .and_then(|rest| {
            rest.chars()
                .next()
                .filter(|separator| matches!(separator, '-' | '_' | ' '))
                .map(|separator| &rest[separator.len_utf8()..])
        })
        .unwrap_or(stem);
    if label.is_empty() {
        "untitled".to_owned()
    } else {
        label.to_owned()
    }
}

fn spans_width(spans: &[Span<'_>]) -> u16 {
    spans.iter().fold(0, |width, span| {
        width.saturating_add(text_width(span.content.as_ref()))
    })
}

fn spans_fit_with_gap(left: &[Span<'_>], right: &[Span<'_>], width: u16) -> bool {
    spans_width(left).saturating_add(if right.is_empty() {
        0
    } else {
        2u16.saturating_add(spans_width(right))
    }) <= width
}

fn fragment_has_note(fragment: &Fragment) -> bool {
    fragment
        .note_content
        .as_deref()
        .is_some_and(|note| !note.is_empty())
}

#[derive(Clone, Copy)]
struct FragmentLineOptions {
    hint: bool,
    note: bool,
    lines: bool,
    title: bool,
    title_width: Option<usize>,
}

fn make_fragment_line(
    theme: TuiTheme,
    current_index: usize,
    snippet: &Snippet,
    options: FragmentLineOptions,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let total = snippet.loaded_fragments.len();
    let current = current_index.saturating_add(1).min(total);
    let fragment = snippet.loaded_fragments.get(current_index);
    let mut left = vec![Span::styled(
        if total > 0 { "+ " } else { "  " },
        Style::default()
            .fg(theme.warning)
            .add_modifier(Modifier::BOLD),
    )];
    left.push(Span::styled(
        if total == 1 { "fragment" } else { "fragments" },
        Style::default().fg(theme.accent),
    ));
    if total > 1 {
        left.push(Span::raw("  "));
        left.push(Span::styled(
            format!("{current}/{total}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if options.title {
        left.push(Span::styled(" · ", Style::default().fg(theme.rule)));
        let label = fragment
            .map(fragment_label)
            .unwrap_or_else(|| "untitled".to_owned());
        left.push(Span::styled(
            options
                .title_width
                .map_or(label.clone(), |width| widgets::truncate_end(&label, width)),
            Style::default()
                .fg(theme.bar_fg)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(fragment) = fragment {
        if options.lines {
            left.push(Span::raw("  "));
            left.push(Span::styled(
                fragment.content.lines().count().to_string(),
                Style::default().fg(theme.accent),
            ));
            left.push(Span::styled(" L", Style::default().fg(theme.muted)));
        }
        if options.note && fragment_has_note(fragment) {
            left.push(Span::styled(
                "  +n",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    let hint = if options.hint && total > 0 {
        vec![
            Span::styled(
                "[ ]",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" switch", Style::default().fg(theme.muted)),
            Span::raw("  "),
            Span::styled(
                "=",
                Style::default()
                    .fg(theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" expand", Style::default().fg(theme.muted)),
        ]
    } else {
        Vec::new()
    };
    (left, hint)
}

fn fragment_line_spans(
    theme: TuiTheme,
    current_index: usize,
    snippet: &Snippet,
    width: u16,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let mut options = FragmentLineOptions {
        hint: true,
        note: true,
        lines: true,
        title: true,
        title_width: None,
    };
    let fits = |left: &[Span<'_>], hint: &[Span<'_>]| spans_fit_with_gap(left, hint, width);
    let mut parts = make_fragment_line(theme, current_index, snippet, options);
    if !fits(&parts.0, &parts.1) {
        options.hint = false;
        parts = make_fragment_line(theme, current_index, snippet, options);
    }
    if !fits(&parts.0, &parts.1) {
        options.note = false;
        parts = make_fragment_line(theme, current_index, snippet, options);
    }
    if !fits(&parts.0, &parts.1) {
        options.lines = false;
        parts = make_fragment_line(theme, current_index, snippet, options);
    }
    if !fits(&parts.0, &parts.1) {
        let title_width = parts
            .0
            .iter()
            .find(|span| span.content.as_ref() == " · ")
            .map_or(8, |_| {
                let natural = snippet
                    .loaded_fragments
                    .get(current_index)
                    .map(fragment_label)
                    .map_or(8, |label| text_width(&label) as usize);
                let without_title = spans_width(&parts.0).saturating_sub(natural as u16);
                usize::from(width.saturating_sub(without_title)).max(8)
            });
        options.title_width = Some(title_width);
        parts = make_fragment_line(theme, current_index, snippet, options);
    }
    if !fits(&parts.0, &parts.1) {
        options.title = false;
        parts = make_fragment_line(theme, current_index, snippet, options);
    }
    parts
}

fn draw_fragment_line(frame: &mut Frame<'_>, app: &App, snippet: &Snippet, area: Rect) {
    let (left, hint) = fragment_line_spans(app.theme, app.fragment_index, snippet, area.width);
    frame.render_widget(Paragraph::new(Line::from(left)), area);
    if !hint.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(hint)).alignment(Alignment::Right),
            area,
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FragmentTreeColumns {
    note: bool,
    lines: bool,
    badge: bool,
    title_width: usize,
}

fn fragment_tree_columns(
    width: u16,
    idx_width: usize,
    line_width: usize,
    natural_title_width: usize,
) -> FragmentTreeColumns {
    let base = 2 + 2 + 1 + idx_width + 2;
    let mut columns = FragmentTreeColumns {
        note: true,
        lines: true,
        badge: true,
        title_width: 8,
    };
    let required = |columns: FragmentTreeColumns| {
        base + 8
            + usize::from(columns.note) * 4
            + usize::from(columns.lines) * (2 + line_width + 2)
            + usize::from(columns.badge) * (2 + 3)
    };
    if required(columns) > width as usize {
        columns.note = false;
    }
    if required(columns) > width as usize {
        columns.lines = false;
    }
    if required(columns) > width as usize {
        columns.badge = false;
    }
    let fixed = base
        + usize::from(columns.note) * 4
        + usize::from(columns.lines) * (2 + line_width + 2)
        + usize::from(columns.badge) * (2 + 3);
    let budget = (width as usize).saturating_sub(fixed);
    columns.title_width = natural_title_width.clamp(8, budget.max(8));
    columns
}

fn draw_fragment_tree(frame: &mut Frame<'_>, app: &mut App, snippet: &Snippet, area: Rect) {
    let total = snippet.loaded_fragments.len();
    let header = vec![
        Span::styled(
            "- ",
            Style::default()
                .fg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if total == 1 { "fragment" } else { "fragments" },
            Style::default().fg(app.theme.accent),
        ),
        Span::raw("  "),
        Span::styled(
            total.to_string(),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    let collapse_hint = vec![
        Span::styled(
            "-",
            Style::default()
                .fg(app.theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" collapse", Style::default().fg(app.theme.muted)),
    ];
    let switch_hint = if total > 1 {
        vec![
            Span::styled(
                "[ ]",
                Style::default()
                    .fg(app.theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" switch", Style::default().fg(app.theme.muted)),
            Span::raw("  "),
        ]
    } else {
        Vec::new()
    };
    let mut hint = switch_hint;
    hint.extend(collapse_hint);
    let header_area = Rect { height: 1, ..area };
    frame.render_widget(Paragraph::new(Line::from(header.clone())), header_area);
    if spans_fit_with_gap(&header, &hint, area.width) {
        frame.render_widget(
            Paragraph::new(Line::from(hint)).alignment(Alignment::Right),
            header_area,
        );
    } else {
        let collapse_hint = vec![
            Span::styled(
                "-",
                Style::default()
                    .fg(app.theme.warning)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" collapse", Style::default().fg(app.theme.muted)),
        ];
        if spans_fit_with_gap(&header, &collapse_hint, area.width) {
            frame.render_widget(
                Paragraph::new(Line::from(collapse_hint)).alignment(Alignment::Right),
                header_area,
            );
        }
    }

    let visible = area.height.saturating_sub(1) as usize;
    let scroll = app
        .fragment_index
        .saturating_sub(visible.saturating_sub(1))
        .min(total.saturating_sub(visible));
    let idx_width = total.to_string().len();
    let line_width = snippet
        .loaded_fragments
        .iter()
        .map(|fragment| fragment.content.lines().count().to_string().len())
        .max()
        .unwrap_or(1)
        .max(3);
    let natural_title_width = snippet
        .loaded_fragments
        .iter()
        .map(fragment_label)
        .map(|label| text_width(&label) as usize)
        .max()
        .unwrap_or(8);
    let columns = fragment_tree_columns(area.width, idx_width, line_width, natural_title_width);

    for (screen_index, index) in (scroll..scroll.saturating_add(visible).min(total)).enumerate() {
        let fragment = &snippet.loaded_fragments[index];
        let selected = index == app.fragment_index;
        let last = index == total.saturating_sub(1);
        let connector = match (last, selected) {
            (true, true) => "└>",
            (true, false) => "└─",
            (false, true) => "├>",
            (false, false) => "├─",
        };
        let label = widgets::truncate_end(&fragment_label(fragment), columns.title_width);
        let label_padding = columns
            .title_width
            .saturating_sub(text_width(&label) as usize);
        let badge = icons::language_badge(&fragment.language);
        let line_count = fragment.content.lines().count();
        let mut spans = vec![
            Span::raw("  "),
            Span::styled(
                connector,
                Style::default()
                    .fg(if selected {
                        app.theme.accent
                    } else {
                        app.theme.rule
                    })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::raw(" "),
            Span::styled(
                format!("{index:>idx_width$}", index = index + 1),
                Style::default()
                    .fg(if selected {
                        app.theme.accent
                    } else {
                        app.theme.muted
                    })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::raw("  "),
            Span::raw(format!("{label}{}", " ".repeat(label_padding))),
        ];
        if columns.badge {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{badge:<3}"),
                Style::default().fg(app.theme.accent_alt),
            ));
        }
        if columns.lines {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{line_count:>line_width$}"),
                Style::default().fg(app.theme.accent),
            ));
            spans.push(Span::styled(" L", Style::default().fg(app.theme.muted)));
        }
        if columns.note {
            spans.push(if fragment_has_note(fragment) {
                Span::styled(
                    "  +n",
                    Style::default()
                        .fg(app.theme.accent_alt)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw("    ")
            });
        }
        let used = spans_width(&spans);
        if used < area.width {
            spans.push(Span::raw(" ".repeat((area.width - used) as usize)));
        }
        if selected {
            for span in &mut spans {
                let preferred = span.style.fg.unwrap_or(app.theme.selection_fg);
                span.style = span
                    .style
                    .fg(app.theme.legible_on(app.theme.selection_bg, preferred))
                    .bg(app.theme.selection_bg)
                    .add_modifier(Modifier::BOLD);
            }
        }
        let y = area.y.saturating_add(1 + screen_index as u16);
        app.layout.fragment_rows.push((y, index));
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                y,
                height: 1,
                ..area
            },
        );
    }
}

fn draw_preview_selection(frame: &mut Frame<'_>, app: &App, area: Rect) {
    for screen_row in 0..area.height {
        let logical_row = app.preview_scroll as usize + screen_row as usize;
        for column in 0..area.width {
            if app.preview_selection.contains(logical_row, column) {
                let cell = &mut frame.buffer_mut()[(area.x + column, area.y + screen_row)];
                cell.fg = app.theme.selection_fg;
                cell.bg = app.theme.selection_bg;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use uuid::Uuid;

    use super::{
        fragment_label, fragment_line_spans, fragment_tree_columns, spans_fit_with_gap, spans_width,
    };
    use crate::domain::{Fingerprint, Fragment, FragmentManifest, Snippet, SnippetManifest};
    use crate::tui::theme::TuiTheme;

    fn fragment(title: &str, file: &str, note: bool) -> Fragment {
        Fragment {
            manifest: FragmentManifest {
                id: Uuid::new_v4(),
                title: title.to_owned(),
                language: "markdown".to_owned(),
                file: file.to_owned(),
                note: None,
                source_language: None,
                extra: toml::Table::new(),
            },
            content: "first\nsecond\n".to_owned(),
            note_content: note.then(|| "note".to_owned()),
            absolute_path: PathBuf::from(file),
        }
    }

    fn snippet() -> Snippet {
        let fragments = vec![
            fragment("output-contract", "fragments/001-output-contract.md", true),
            fragment("severity-rubric", "fragments/002-severity-rubric.md", false),
        ];
        Snippet {
            manifest: SnippetManifest {
                schema_version: 1,
                id: Uuid::new_v4(),
                title: "Code Review".to_owned(),
                tags: Vec::new(),
                pinned: false,
                locked: false,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                source: None,
                remotes: Vec::new(),
                fragments: fragments
                    .iter()
                    .map(|value| value.manifest.clone())
                    .collect(),
                extra: toml::Table::new(),
            },
            readme: None,
            folder: String::new(),
            package_path: PathBuf::new(),
            modified_at: None,
            fingerprint: Fingerprint("test".to_owned()),
            loaded_fragments: fragments,
        }
    }

    fn text(spans: &[ratatui::text::Span<'_>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn explicit_titles_are_kept_verbatim_and_filenames_lose_their_index_prefix() {
        assert_eq!(
            fragment_label(&fragment(
                "01-explicit",
                "fragments/999-ignored-title.md",
                false
            )),
            "01-explicit"
        );
        assert_eq!(
            fragment_label(&fragment("  ", "fragments/001-Code Review.md", false)),
            "Code Review"
        );
        assert_eq!(fragment_label(&fragment("", "", false)), "untitled");
    }

    #[test]
    fn a_narrow_fragment_line_drops_hint_then_note_then_metadata() {
        let snippet = snippet();
        let theme = TuiTheme::from(&crate::theme::load("dark-default").unwrap());
        let (full_left, full_hint) = fragment_line_spans(theme, 0, &snippet, u16::MAX);
        let full_width = spans_width(&full_left) + 2 + spans_width(&full_hint);
        assert!(!text(&full_left).contains("md"));

        let (without_hint, hint) = fragment_line_spans(theme, 0, &snippet, full_width - 1);
        assert!(hint.is_empty());
        assert!(text(&without_hint).contains("+n"));
        assert!(text(&without_hint).contains(" L"));

        let (without_note, _) =
            fragment_line_spans(theme, 0, &snippet, spans_width(&without_hint) - 1);
        assert!(!text(&without_note).contains("+n"));
        assert!(text(&without_note).contains(" L"));

        let (without_lines, _) =
            fragment_line_spans(theme, 0, &snippet, spans_width(&without_note) - 1);
        assert!(!text(&without_lines).contains(" L"));
        assert!(text(&without_lines).contains("output-contract"));
    }

    #[test]
    fn a_narrow_fragment_tree_header_drops_its_hint_before_overlap() {
        let header = vec![ratatui::text::Span::raw("- fragments  5")];
        let hint = vec![ratatui::text::Span::raw("- collapse")];
        let needed = spans_width(&header) + 2 + spans_width(&hint);

        assert!(spans_fit_with_gap(&header, &hint, needed));
        assert!(!spans_fit_with_gap(&header, &hint, needed - 1));
    }

    #[test]
    fn narrow_fragment_tree_rows_drop_note_then_lines_then_badge() {
        let idx_width = 1;
        let line_width = 3;
        let full_width = 2 + 2 + 1 + idx_width + 2 + 8 + 4 + (2 + line_width + 2) + (2 + 3);

        let full = fragment_tree_columns(full_width as u16, idx_width, line_width, 20);
        assert!(full.note && full.lines && full.badge);

        let without_note =
            fragment_tree_columns((full_width - 1) as u16, idx_width, line_width, 20);
        assert!(!without_note.note && without_note.lines && without_note.badge);

        let lines_threshold = full_width - 4;
        let without_lines =
            fragment_tree_columns((lines_threshold - 1) as u16, idx_width, line_width, 20);
        assert!(!without_lines.note && !without_lines.lines && without_lines.badge);

        let badge_threshold = lines_threshold - (2 + line_width + 2);
        let without_badge =
            fragment_tree_columns((badge_threshold - 1) as u16, idx_width, line_width, 20);
        assert!(!without_badge.note && !without_badge.lines && !without_badge.badge);
    }
}
