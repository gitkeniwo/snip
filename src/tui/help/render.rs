use std::collections::HashSet;
use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Padding, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState,
};

use super::{HelpGroup, HelpScope, HelpSort, VisibleHelpRow, help_chord};
use crate::keys::{Keymap, Mode};
use crate::tui::app::App;
use crate::tui::command::CommandId;
use crate::tui::selection::text_width;
use crate::tui::theme::TuiTheme;
use crate::tui::widgets;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrappedLine {
    pub text: String,
    pub source: Range<usize>,
}

pub struct HelpRenderPlan {
    pub lines: Vec<Line<'static>>,
    pub row_line_ranges: Vec<Range<usize>>,
}

pub fn wrap_words(text: &str, width: usize) -> Vec<WrappedLine> {
    if text.is_empty() {
        return vec![WrappedLine {
            text: String::new(),
            source: 0..0,
        }];
    }
    let width = width.max(1);
    let chars = text.chars().collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        while start < chars.len() && chars[start].is_whitespace() {
            start += 1;
        }
        if start == chars.len() {
            break;
        }
        let mut end = start;
        let mut used = 0usize;
        let mut last_space = None;
        while end < chars.len() {
            let character_width = text_width(&chars[end].to_string()) as usize;
            if used + character_width > width && end > start {
                break;
            }
            used += character_width;
            if chars[end].is_whitespace() {
                last_space = Some(end);
            }
            end += 1;
            if used >= width {
                break;
            }
        }
        if end < chars.len()
            && let Some(space) = last_space.filter(|space| *space > start)
        {
            end = space;
        }
        let trimmed_end = (start..end)
            .rev()
            .find(|index| !chars[*index].is_whitespace())
            .map_or(start, |index| index + 1);
        lines.push(WrappedLine {
            text: chars[start..trimmed_end].iter().collect(),
            source: start..trimmed_end,
        });
        start = end.max(start + 1);
    }
    lines
}

pub fn build_render_plan(width: usize, rows: &[VisibleHelpRow], theme: TuiTheme) -> HelpRenderPlan {
    build_render_plan_with_selection(width, rows, theme, usize::MAX)
}

fn build_render_plan_with_selection(
    width: usize,
    rows: &[VisibleHelpRow],
    theme: TuiTheme,
    selected: usize,
) -> HelpRenderPlan {
    let key_w = rows
        .iter()
        .map(|row| text_width(&key_label(row).0) as usize)
        .max()
        .unwrap_or(1)
        .clamp(1, 20);
    let slug_w = rows
        .iter()
        .map(|row| text_width(row.row.slug) as usize)
        .max()
        .unwrap_or(1)
        .clamp(1, 32);
    let wide_desc_w = width.saturating_sub(key_w + slug_w + 2);
    let wide = wide_desc_w >= 24;
    let mut lines = Vec::new();
    let mut ranges = vec![0..0; rows.len()];
    let mut previous_group = None;
    for (index, visible) in rows.iter().enumerate() {
        if previous_group != Some(visible.row.display_group) {
            if !lines.is_empty() {
                lines.push(Line::raw(""));
            }
            let count = rows
                .iter()
                .filter(|candidate| candidate.row.display_group == visible.row.display_group)
                .count();
            lines.push(group_header(
                visible.row.display_group,
                count,
                &visible.matched.group_indices,
                width,
                theme,
            ));
            previous_group = Some(visible.row.display_group);
        }
        let start = lines.len();
        let selected_row = index == selected;
        if wide {
            lines.extend(wide_row(
                visible,
                key_w,
                slug_w,
                wide_desc_w,
                width,
                theme,
                selected_row,
            ));
        } else {
            lines.extend(narrow_row(visible, width, theme, selected_row));
        }
        ranges[index] = start..lines.len();
    }
    HelpRenderPlan {
        lines,
        row_line_ranges: ranges,
    }
}

pub fn draw_help(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let floor = area.width.saturating_sub(4).min(80);
    let popup_width = area
        .width
        .saturating_mul(7)
        .saturating_div(10)
        .max(floor)
        .min(100)
        .min(area.width);
    let content_width = popup_width.saturating_sub(6) as usize;
    let plan = build_render_plan_with_selection(
        content_width.saturating_sub(1),
        app.help.visible_rows(),
        app.theme,
        app.help.selected,
    );
    let chrome = 4 + usize::from(app.help.filtering);
    let needed = plan.lines.len().saturating_add(chrome);
    let popup_height = u16::try_from(needed)
        .unwrap_or(u16::MAX)
        .min(area.height.saturating_mul(7).saturating_div(10))
        .max(12.min(area.height))
        .min(area.height);
    let popup = widgets::centered_rect(popup_width, popup_height, area);
    frame.render_widget(Clear, popup);
    widgets::fill_surface(frame, popup, app.theme);

    let context = app.help.scope_stack.first().copied();
    let title = title_line(app.help.scope, context, app.help.sort, app.theme);
    let count = if app.help.filter.value.is_empty() {
        format!(
            " {}/{} ",
            app.help.selected.saturating_add(1).min(app.help.count()),
            app.help.count()
        )
    } else {
        format!(
            " {}/{} matches ",
            app.help.selected.saturating_add(1).min(app.help.count()),
            app.help.count()
        )
    };
    let hint_width = (popup_width as usize)
        .saturating_sub(2)
        .saturating_sub(text_width(&count) as usize)
        .saturating_sub(1);
    let left_hint = bottom_hint(app, hint_width);
    let block = Block::default()
        .title(title.centered())
        .title_bottom(left_hint.left_aligned())
        .title_bottom(Line::from(count).right_aligned())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .padding(Padding::new(2, 2, 1, 1));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let filter_height = u16::from(app.help.filtering);
    if app.help.filtering {
        frame.render_widget(
            Paragraph::new(filter_line(app)),
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            },
        );
    }
    let body = Rect {
        x: inner.x,
        y: inner.y.saturating_add(filter_height),
        width: inner.width,
        height: inner.height.saturating_sub(filter_height),
    };
    let line_count = plan.lines.len();
    app.help
        .update_layout(body.height as usize, plan.row_line_ranges);
    let visible = plan
        .lines
        .into_iter()
        .skip(app.help.scroll)
        .take(body.height as usize)
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(visible), body);
    if line_count > body.height as usize {
        let mut scrollbar = ScrollbarState::new(scroll_positions(line_count, body.height as usize))
            .position(app.help.scroll)
            .viewport_content_length(body.height as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            body,
            &mut scrollbar,
        );
    }
}

fn wide_row(
    visible: &VisibleHelpRow,
    key_w: usize,
    slug_w: usize,
    desc_w: usize,
    total_w: usize,
    theme: TuiTheme,
    selected: bool,
) -> Vec<Line<'static>> {
    let (key, marker_index) = key_label(visible);
    let keys = wrap_words(&key, key_w);
    let slugs = wrap_words(visible.row.slug, slug_w);
    let descriptions = wrap_words(visible.row.description, desc_w);
    let line_count = keys.len().max(slugs.len()).max(descriptions.len());
    let base = row_base_style(theme, selected);
    let mut lines = (0..line_count)
        .map(|line_index| {
            let mut spans = Vec::new();
            spans.extend(cell_spans_with_warning(
                keys.get(line_index),
                &visible.matched.key_indices,
                key_w,
                key_style(visible.row.display_group, theme, base, selected),
                theme,
                marker_index,
                selected,
            ));
            spans.push(Span::styled(" ", base));
            spans.extend(cell_spans(
                slugs.get(line_index),
                &visible.matched.slug_indices,
                slug_w,
                secondary_style(theme, base, selected),
                theme,
                selected,
            ));
            spans.push(Span::styled(" ", base));
            spans.extend(cell_spans(
                descriptions.get(line_index),
                &visible.matched.description_indices,
                desc_w,
                base,
                theme,
                selected,
            ));
            pad_line(&mut spans, total_w, base);
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    if selected {
        lines.extend(note_lines(visible, total_w, base));
    }
    lines
}

fn narrow_row(
    visible: &VisibleHelpRow,
    width: usize,
    theme: TuiTheme,
    selected: bool,
) -> Vec<Line<'static>> {
    let (key, marker_index) = key_label(visible);
    let key_w = (text_width(&key) as usize).min(20).min(width / 2).max(1);
    let desc_w = width.saturating_sub(key_w + 1).max(1);
    let keys = wrap_words(&key, key_w);
    let descriptions = wrap_words(visible.row.description, desc_w);
    let line_count = keys.len().max(descriptions.len());
    let base = row_base_style(theme, selected);
    let mut lines = (0..line_count)
        .map(|line_index| {
            let mut spans = cell_spans_with_warning(
                keys.get(line_index),
                &visible.matched.key_indices,
                key_w,
                key_style(visible.row.display_group, theme, base, selected),
                theme,
                marker_index,
                selected,
            );
            spans.push(Span::styled(" ", base));
            spans.extend(cell_spans(
                descriptions.get(line_index),
                &visible.matched.description_indices,
                desc_w,
                base,
                theme,
                selected,
            ));
            pad_line(&mut spans, width, base);
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    if !visible.row.slug.is_empty() {
        for wrapped in wrap_words(visible.row.slug, width.saturating_sub(key_w + 1)) {
            let mut spans = vec![Span::styled(" ".repeat(key_w + 1), base)];
            spans.extend(cell_spans(
                Some(&wrapped),
                &visible.matched.slug_indices,
                width.saturating_sub(key_w + 1),
                secondary_style(theme, base, selected),
                theme,
                selected,
            ));
            pad_line(&mut spans, width, base);
            lines.push(Line::from(spans));
        }
    }
    if selected {
        lines.extend(note_lines(visible, width, base));
    }
    lines
}

fn key_label(visible: &VisibleHelpRow) -> (String, Option<usize>) {
    let mut key = visible.row.key.display();
    if !visible.row.user_modified {
        return (key, None);
    }
    key.push(' ');
    let marker_index = key.chars().count();
    key.push('*');
    (key, Some(marker_index))
}

fn cell_spans(
    wrapped: Option<&WrappedLine>,
    indices: &[u32],
    width: usize,
    base: Style,
    theme: TuiTheme,
    selected: bool,
) -> Vec<Span<'static>> {
    cell_spans_with_warning(wrapped, indices, width, base, theme, None, selected)
}

fn cell_spans_with_warning(
    wrapped: Option<&WrappedLine>,
    indices: &[u32],
    width: usize,
    base: Style,
    theme: TuiTheme,
    warning_source_index: Option<usize>,
    selected: bool,
) -> Vec<Span<'static>> {
    let Some(wrapped) = wrapped else {
        return vec![Span::styled(" ".repeat(width), base)];
    };
    let matches = indices.iter().copied().collect::<HashSet<_>>();
    let mut spans = wrapped
        .text
        .chars()
        .enumerate()
        .map(|(offset, character)| {
            let source_index = wrapped.source.start + offset;
            let style = if warning_source_index == Some(source_index) {
                base.fg(row_semantic_color(theme, theme.warning, selected))
                    .add_modifier(Modifier::BOLD)
            } else if matches.contains(&(source_index as u32)) {
                base.fg(row_semantic_color(theme, theme.accent, selected))
                    .add_modifier(Modifier::BOLD)
            } else {
                base
            };
            Span::styled(character.to_string(), style)
        })
        .collect::<Vec<_>>();
    let used = text_width(&wrapped.text) as usize;
    spans.push(Span::styled(" ".repeat(width.saturating_sub(used)), base));
    spans
}

fn note_lines(visible: &VisibleHelpRow, width: usize, base: Style) -> Vec<Line<'static>> {
    let note = if let Some(hidden) = &visible.matched.hidden_reason {
        format!("matched {}: {}", hidden.field, hidden.value)
    } else if visible.row.user_modified {
        "custom binding".to_owned()
    } else {
        return Vec::new();
    };
    let indent = 2.min(width.saturating_sub(1));
    let note_width = width.saturating_sub(indent).max(1);
    wrap_words(&note, note_width)
        .into_iter()
        .map(|wrapped| {
            let mut spans = vec![Span::styled(" ".repeat(indent), base)];
            spans.push(Span::styled(wrapped.text, base));
            pad_line(&mut spans, width, base);
            Line::from(spans)
        })
        .collect()
}

fn row_base_style(theme: TuiTheme, selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(theme.legible_on(theme.selection_bg, theme.selection_fg))
            .bg(theme.selection_bg)
    } else {
        Style::default()
    }
}

fn key_style(group: HelpGroup, theme: TuiTheme, base: Style, selected: bool) -> Style {
    if selected {
        base.add_modifier(Modifier::BOLD)
    } else {
        base.fg(group_color(group, theme))
            .add_modifier(Modifier::BOLD)
    }
}

fn secondary_style(theme: TuiTheme, base: Style, selected: bool) -> Style {
    if selected { base } else { base.fg(theme.muted) }
}

fn row_semantic_color(theme: TuiTheme, preferred: Color, selected: bool) -> Color {
    if selected {
        theme.legible_on(theme.selection_bg, preferred)
    } else {
        preferred
    }
}

fn scroll_positions(line_count: usize, viewport_height: usize) -> usize {
    line_count.saturating_sub(viewport_height) + 1
}

fn pad_line(spans: &mut Vec<Span<'static>>, width: usize, style: Style) {
    let used = spans
        .iter()
        .map(|span| text_width(span.content.as_ref()) as usize)
        .sum::<usize>();
    spans.push(Span::styled(" ".repeat(width.saturating_sub(used)), style));
}

fn group_header(
    group: HelpGroup,
    count: usize,
    indices: &[u32],
    width: usize,
    theme: TuiTheme,
) -> Line<'static> {
    let label = format!("{} ({count})", group.label());
    let color = group_color(group, theme);
    let matches = indices.iter().copied().collect::<HashSet<_>>();
    let mut spans = vec![Span::styled("── ", Style::default().fg(theme.rule))];
    spans.extend(label.chars().enumerate().map(|(index, character)| {
        let style = Style::default()
            .fg(if matches.contains(&(index as u32)) {
                theme.warning
            } else {
                color
            })
            .add_modifier(Modifier::BOLD);
        Span::styled(character.to_string(), style)
    }));
    spans.push(Span::styled(" ──", Style::default().fg(theme.rule)));
    let left = width.saturating_sub(Line::from(spans.clone()).width()) / 2;
    spans.insert(0, Span::raw(" ".repeat(left)));
    Line::from(spans).alignment(Alignment::Left)
}

fn group_color(group: HelpGroup, theme: TuiTheme) -> Color {
    match group {
        HelpGroup::Mode(Mode::Global) | HelpGroup::Inherited => theme.accent,
        HelpGroup::Mode(Mode::Sidebar) => theme.tag,
        HelpGroup::Mode(Mode::List) | HelpGroup::Mode(Mode::Preview) => theme.accent_alt,
        HelpGroup::Mode(Mode::Fragment) | HelpGroup::Mode(Mode::FragmentGrab) => theme.warning,
        HelpGroup::Mode(Mode::Trash) => theme.error,
        HelpGroup::Mode(Mode::Git) => theme.accent_alt,
        HelpGroup::Mode(Mode::Gist) | HelpGroup::Mode(Mode::Search) => theme.success,
        HelpGroup::Mode(Mode::Help) | HelpGroup::HelpControls => theme.muted,
        HelpGroup::Numbers | HelpGroup::Mouse | HelpGroup::System => theme.success,
    }
}

fn filter_line(app: &App) -> Line<'static> {
    let cursor = app
        .help
        .filter
        .cursor
        .min(app.help.filter.value.chars().count());
    let mut spans = vec![Span::styled(
        "/",
        Style::default()
            .fg(app.theme.accent)
            .add_modifier(Modifier::BOLD),
    )];
    for (index, character) in app.help.filter.value.chars().enumerate() {
        if index == cursor {
            spans.push(Span::styled(" ", Style::default().bg(app.theme.accent)));
        }
        spans.push(Span::raw(character.to_string()));
    }
    if cursor == app.help.filter.value.chars().count() {
        spans.push(Span::styled(" ", Style::default().bg(app.theme.accent)));
    }
    Line::from(spans)
}

fn title_line(
    scope: HelpScope,
    context: Option<Mode>,
    sort: HelpSort,
    theme: TuiTheme,
) -> Line<'static> {
    let scope = match scope {
        HelpScope::Context => context.map(context_label).unwrap_or("Context"),
        HelpScope::All => "All modes",
    };
    let sort = match sort {
        HelpSort::Key => "Key",
        HelpSort::Action => "Action",
    };
    let normal = theme.foreground.unwrap_or(Color::Reset);
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "Help",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" › ", Style::default().fg(theme.rule)),
        Span::styled(
            scope.to_owned(),
            Style::default().fg(normal).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(theme.rule)),
        Span::styled("Sort: ", Style::default().fg(theme.muted)),
        Span::styled(
            sort.to_owned(),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ↑", Style::default().fg(theme.muted)),
        Span::raw(" "),
    ])
}

fn bottom_hint(app: &App, max_width: usize) -> Line<'static> {
    bottom_hint_line(
        app.help.scope,
        app.help.scope_stack.first().copied(),
        app.help.sort,
        app.help.filtering,
        app.theme,
        &app.keymap,
        max_width,
    )
}

fn bottom_hint_line(
    scope: HelpScope,
    context: Option<Mode>,
    sort: HelpSort,
    filtering: bool,
    theme: TuiTheme,
    keymap: &Keymap,
    max_width: usize,
) -> Line<'static> {
    let items = if filtering {
        vec![
            ("Esc".to_owned(), "Clear".to_owned()),
            ("Enter".to_owned(), "Keep filter".to_owned()),
            ("↑/↓".to_owned(), "Select".to_owned()),
        ]
    } else {
        let context = context.map(context_label).unwrap_or("Context");
        let scope_label = match scope {
            HelpScope::Context => "All modes".to_owned(),
            HelpScope::All => format!("{context} only"),
        };
        let sort_label = match sort {
            HelpSort::Key => "Sort by action",
            HelpSort::Action => "Sort by key",
        };
        [
            (CommandId::HelpToggleScope, scope_label),
            (CommandId::HelpCycleSort, sort_label.to_owned()),
            (CommandId::HelpFilter, "Filter".to_owned()),
        ]
        .into_iter()
        .filter_map(|(id, label)| {
            let keys = keymap
                .chords_for(&[Mode::Help], id)
                .into_iter()
                .map(help_chord)
                .collect::<Vec<_>>()
                .join("/");
            (!keys.is_empty()).then_some((keys, label))
        })
        .collect()
    };
    hint_line(items, theme, max_width)
}

fn hint_line(mut items: Vec<(String, String)>, theme: TuiTheme, max_width: usize) -> Line<'static> {
    loop {
        let normal = theme.foreground.unwrap_or(Color::Reset);
        let mut spans = vec![Span::raw(" ")];
        for (index, (keys, label)) in items.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(" · ", Style::default().fg(theme.rule)));
            }
            spans.push(Span::styled(
                format!("[{keys}]"),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(label.clone(), Style::default().fg(normal)));
        }
        spans.push(Span::raw(" "));
        let line = Line::from(spans);
        if line.width() <= max_width || items.is_empty() {
            return line;
        }
        items.pop();
    }
}

fn context_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Global => "Global",
        Mode::Sidebar => "Library pane",
        Mode::List => "Snippet list",
        Mode::Preview | Mode::Fragment => "Preview",
        Mode::FragmentGrab => "Fragment move",
        Mode::Trash => "Trash",
        Mode::Help => "Help",
        Mode::Git => "Git console",
        Mode::Gist => "Gist panel",
        Mode::Search => "Search",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::widgets::StatefulWidget;

    use crate::keys::{Keymap, Mode};
    use crate::tui::help::{HelpState, HiddenMatch};
    use crate::tui::theme::Appearance;

    fn normalized(text: &str) -> String {
        text.chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    fn row_text(plan: &HelpRenderPlan, index: usize) -> String {
        plan.lines[plan.row_line_ranges[index].clone()]
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn luminance(color: Color) -> Option<f64> {
        let [red, green, blue] = match color {
            Color::Rgb(red, green, blue) => [red, green, blue],
            Color::Indexed(index @ 16..=231) => {
                const COMPONENTS: [u8; 6] = [0, 95, 135, 175, 215, 255];
                let index = index - 16;
                [
                    COMPONENTS[usize::from(index / 36)],
                    COMPONENTS[usize::from(index % 36 / 6)],
                    COMPONENTS[usize::from(index % 6)],
                ]
            }
            Color::Indexed(index @ 232..=255) => {
                let value = 8 + 10 * (index - 232);
                [value, value, value]
            }
            _ => return None,
        };
        let channel = |value: u8| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        Some(0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue))
    }

    fn contrast(foreground: Color, background: Color) -> Option<f64> {
        let foreground = luminance(foreground)?;
        let background = luminance(background)?;
        Some((foreground.max(background) + 0.05) / (foreground.min(background) + 0.05))
    }

    #[test]
    fn wrapping_preserves_source_ranges_and_never_ellipsizes() {
        let text = "Move 你好 through one-unbreakable-long-word safely";
        let lines = wrap_words(text, 9);
        assert!(!lines.iter().any(|line| line.text.contains('…')));
        for line in lines {
            let source = text.chars().collect::<Vec<_>>()[line.source]
                .iter()
                .collect::<String>();
            assert_eq!(line.text, source);
        }
    }

    #[test]
    fn real_rows_never_ellipsize_and_narrow_rows_put_the_slug_below() {
        let keymap = Keymap::defaults();
        let mut state = HelpState::default();
        state.open(vec![Mode::List, Mode::Global], &keymap, None);
        let theme = TuiTheme::default_for(Appearance::Dark);
        let plan = build_render_plan(54, state.visible_rows(), theme);
        assert!(
            plan.lines
                .iter()
                .all(|line| !line.to_string().contains('…'))
        );

        let index = state
            .visible_rows()
            .iter()
            .position(|row| row.row.slug == "snippet.rename")
            .unwrap();
        let range = plan.row_line_ranges[index].clone();
        let rendered = plan.lines[range]
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>();
        assert!(!rendered[0].contains("snippet.rename"));
        assert!(
            rendered
                .iter()
                .skip(1)
                .any(|line| line.contains("snippet.rename"))
        );
    }

    #[test]
    fn selected_hidden_match_note_wraps_in_wide_and_narrow_layouts() {
        let keymap = Keymap::defaults();
        let mut state = HelpState::default();
        state.open(vec![Mode::List, Mode::Global], &keymap, None);
        let mut row = state.visible_rows()[0].clone();
        let value = "强制主题匹配字段非常长而且不能因为终端宽度不足就消失".repeat(3);
        row.matched.hidden_reason = Some(HiddenMatch {
            field: "keyword",
            value: value.clone(),
        });
        let note = format!("matched keyword: {value}");
        let theme = TuiTheme::default_for(Appearance::Dark);

        for width in [100, 30] {
            let plan = build_render_plan_with_selection(width, &[row.clone()], theme, 0);
            let rendered = row_text(&plan, 0);
            assert!(
                normalized(&rendered).contains(&normalized(&note)),
                "width {width}: {rendered}"
            );
            assert!(plan.row_line_ranges[0].len() > 2, "width {width}");
        }
    }

    #[test]
    fn custom_bindings_keep_warning_marker_and_selected_note() {
        let keymap = Keymap::defaults();
        let mut state = HelpState::default();
        state.open(vec![Mode::List, Mode::Global], &keymap, None);
        let mut row = state.visible_rows()[0].clone();
        row.row.user_modified = true;
        let theme = TuiTheme::default_for(Appearance::Dark);

        for width in [100, 30] {
            let unselected = build_render_plan(width, &[row.clone()], theme);
            assert!(row_text(&unselected, 0).contains('*'), "width {width}");
            assert!(
                unselected.lines[unselected.row_line_ranges[0].clone()]
                    .iter()
                    .flat_map(|line| line.spans.iter())
                    .any(|span| span.content == "*" && span.style.fg == Some(theme.warning)),
                "width {width}"
            );

            let selected = build_render_plan_with_selection(width, &[row.clone()], theme, 0);
            assert!(
                row_text(&selected, 0).contains("custom binding"),
                "width {width}"
            );
        }
    }

    #[test]
    fn scrollbar_thumb_reaches_bottom_at_last_legal_position() {
        let line_count = 50;
        let viewport_height = 10;
        let mut state = ScrollbarState::new(scroll_positions(line_count, viewport_height))
            .position(line_count - viewport_height)
            .viewport_content_length(viewport_height);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("║"))
            .thumb_symbol("█");
        let area = Rect::new(0, 0, 1, viewport_height as u16);
        let mut buffer = Buffer::empty(area);

        scrollbar.render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(0, viewport_height as u16 - 1)].symbol(), "█");
    }

    #[test]
    fn selected_help_spans_are_legible_in_every_builtin_theme() {
        let keymap = Keymap::defaults();
        let mut state = HelpState::default();
        state.open(vec![Mode::List, Mode::Global], &keymap, None);
        let mut row = state.visible_rows()[0].clone();
        row.row.user_modified = true;
        row.matched.key_indices = vec![0];
        row.matched.slug_indices = vec![0];
        row.matched.description_indices = vec![0];
        row.matched.hidden_reason = Some(HiddenMatch {
            field: "keyword",
            value: "force".to_owned(),
        });
        let expected_key = row.row.key.display();
        let expected_slug = row.row.slug;
        let expected_description = row.row.description;

        for (name, _) in crate::theme::builtin::THEMES {
            let theme = TuiTheme::from(&crate::theme::load(name).unwrap());
            let plan = build_render_plan_with_selection(100, &[row.clone()], theme, 0);
            let range = plan.row_line_ranges[0].clone();
            let rendered = row_text(&plan, 0);
            for expected in [
                expected_key.as_str(),
                expected_slug,
                expected_description,
                "*",
                "matched keyword: force",
            ] {
                assert!(
                    rendered.contains(expected),
                    "theme {name}: missing {expected}"
                );
            }

            let spans = plan.lines[range]
                .iter()
                .flat_map(|line| line.spans.iter())
                .filter(|span| !span.content.trim().is_empty())
                .collect::<Vec<_>>();
            assert!(
                spans.iter().any(|span| span.content == "*"),
                "theme {name}: marker missing"
            );
            assert!(
                spans
                    .iter()
                    .filter(|span| {
                        span.style.fg == Some(theme.legible_on(theme.selection_bg, theme.accent))
                    })
                    .count()
                    >= 3,
                "theme {name}: key/slug/description match highlighting missing"
            );
            for span in spans {
                assert_eq!(
                    span.style.bg,
                    Some(theme.selection_bg),
                    "theme {name}: {:?}",
                    span.content
                );
                let foreground = span.style.fg.expect("selected text needs a foreground");
                let ratio = contrast(foreground, theme.selection_bg)
                    .unwrap_or_else(|| panic!("theme {name}: non-measurable selected color"));
                assert!(
                    ratio >= 4.5,
                    "theme {name}: {:?} has {ratio:.2}:1 contrast",
                    span.content
                );
            }
        }
    }

    #[test]
    fn title_and_hints_track_scope_context_and_sort() {
        let theme = TuiTheme::default_for(Appearance::Dark);
        let keymap = Keymap::defaults();
        let context_title = title_line(
            HelpScope::Context,
            Some(Mode::Preview),
            HelpSort::Key,
            theme,
        );
        let context_hint = bottom_hint_line(
            HelpScope::Context,
            Some(Mode::Preview),
            HelpSort::Key,
            false,
            theme,
            &keymap,
            100,
        );
        assert_eq!(
            context_title.to_string().trim(),
            "Help › Preview · Sort: Key ↑"
        );
        assert_eq!(
            context_hint.to_string().trim(),
            "[a] All modes · [s] Sort by action · [/] Filter"
        );

        let all_title = title_line(HelpScope::All, Some(Mode::Preview), HelpSort::Action, theme);
        let all_hint = bottom_hint_line(
            HelpScope::All,
            Some(Mode::Preview),
            HelpSort::Action,
            false,
            theme,
            &keymap,
            100,
        );
        assert_eq!(
            all_title.to_string().trim(),
            "Help › All modes · Sort: Action ↑"
        );
        assert_eq!(
            all_hint.to_string().trim(),
            "[a] Preview only · [s] Sort by key · [/] Filter"
        );

        let filtering = bottom_hint_line(
            HelpScope::Context,
            Some(Mode::Preview),
            HelpSort::Key,
            true,
            theme,
            &keymap,
            100,
        );
        assert_eq!(
            filtering.to_string().trim(),
            "[Esc] Clear · [Enter] Keep filter · [↑/↓] Select"
        );
    }

    #[test]
    fn bottom_hints_follow_rebindings_and_hide_unbound_commands() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("keys.toml");
        std::fs::write(
            &path,
            r#"
                [help]
                "help.toggle-scope" = "x"
                "help.cycle-sort" = []
                "help.filter" = "f"
            "#,
        )
        .unwrap();
        let (keymap, diagnostics) = Keymap::load_from(&path).unwrap();
        assert!(diagnostics.is_empty());
        let theme = TuiTheme::default_for(Appearance::Dark);
        let hint = bottom_hint_line(
            HelpScope::Context,
            Some(Mode::Sidebar),
            HelpSort::Key,
            false,
            theme,
            &keymap,
            100,
        );
        let rendered = hint.to_string();

        assert!(rendered.contains("[x] All modes"));
        assert!(rendered.contains("[f] Filter"));
        assert!(!rendered.contains("[a]"));
        assert!(!rendered.contains("[s]"));
        assert!(!rendered.contains("Sort by action"));
    }

    #[test]
    fn eighty_column_chrome_budget_keeps_hint_clear_of_count() {
        let theme = TuiTheme::default_for(Appearance::Dark);
        let keymap = Keymap::defaults();
        let popup_width = 76usize;
        let count = " 1/50 ";
        let available = popup_width - 2 - text_width(count) as usize - 1;
        let title = title_line(HelpScope::All, Some(Mode::Preview), HelpSort::Action, theme);
        let hint = bottom_hint_line(
            HelpScope::All,
            Some(Mode::Preview),
            HelpSort::Action,
            false,
            theme,
            &keymap,
            available,
        );

        assert_eq!(
            hint.to_string().trim(),
            "[a] Preview only · [s] Sort by key · [/] Filter"
        );
        assert!(title.width() <= popup_width - 2);
        assert!(hint.width() + (text_width(count) as usize) < popup_width - 2);
    }
}
