use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::app::App;
use super::theme::TuiTheme;
use super::widgets;

pub fn draw_top_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(
        Block::default().style(Style::default().fg(app.theme.bar_fg).bg(app.theme.bar_bg)),
        area,
    );
    let counts = if let Some(index) = app.list_state.selected() {
        let fragments = app
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        format!(
            "{}/{} · {}/{}",
            index + 1,
            app.visible.len(),
            app.fragment_index.saturating_add(1).min(fragments),
            fragments
        )
    } else {
        format!("0/{}", app.visible.len())
    };
    let brand_color = if app.modal.is_some() {
        app.theme.accent_alt
    } else if app.search.active {
        app.theme.warning
    } else {
        app.theme.pill_primary
    };
    let right = top_position_pill(app.sort.indicator(), &counts, app.theme);
    let right_width = right.width().min(area.width as usize) as u16;
    let regions = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(2),
        Constraint::Length(right_width),
    ])
    .split(area);
    let left = top_context_pill(app, regions[0].width as usize, brand_color);
    frame.render_widget(Paragraph::new(left), regions[0]);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        regions[2],
    );
}

fn top_context_pill(app: &App, width: usize, primary: ratatui::style::Color) -> Line<'static> {
    let primary_style = Style::default()
        .fg(app.theme.selection_fg)
        .bg(primary)
        .add_modifier(Modifier::BOLD);
    if width < 15 {
        return Line::from(vec![
            widgets::pill_cap(widgets::PILL_OPEN, primary, app.theme.bar_bg),
            Span::styled(" snip ", primary_style),
            widgets::pill_cap(widgets::PILL_CLOSE, primary, app.theme.bar_bg),
        ]);
    }

    let secondary = app.theme.pill_secondary;
    let secondary_style = Style::default().fg(app.theme.bar_fg).bg(secondary);
    let mut spans = vec![
        widgets::pill_cap(widgets::PILL_OPEN, primary, app.theme.bar_bg),
        Span::styled(" snip ", primary_style),
        widgets::pill_cap(widgets::PILL_CLOSE, primary, secondary),
        Span::styled(" ", secondary_style),
    ];
    spans.extend(breadcrumb_spans(
        app,
        width.saturating_sub(11),
        secondary_style,
    ));
    spans.push(Span::styled(" ", secondary_style));
    spans.push(widgets::pill_cap(
        widgets::PILL_CLOSE,
        secondary,
        app.theme.bar_bg,
    ));
    Line::from(spans)
}

fn top_position_pill(sort: Option<&str>, counts: &str, theme: TuiTheme) -> Line<'static> {
    let primary = theme.pill_primary;
    let secondary = theme.pill_secondary;
    let mut spans = Vec::new();
    if let Some(sort) = sort {
        spans.push(widgets::pill_cap(
            widgets::PILL_OPEN,
            secondary,
            theme.bar_bg,
        ));
        spans.push(Span::styled(
            format!(" {sort} "),
            Style::default()
                .fg(primary)
                .bg(secondary)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(widgets::pill_cap(widgets::PILL_CLOSE, secondary, primary));
    } else {
        spans.push(widgets::pill_cap(widgets::PILL_OPEN, primary, theme.bar_bg));
    }
    spans.push(Span::styled(
        format!(" {counts} "),
        Style::default()
            .fg(theme.selection_fg)
            .bg(primary)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(widgets::pill_cap(
        widgets::PILL_CLOSE,
        primary,
        theme.bar_bg,
    ));
    Line::from(spans)
}

fn breadcrumb_spans(app: &App, width: usize, base: Style) -> Vec<Span<'static>> {
    let mut segments = if let Some(folder) = &app.filter.folder {
        folder.split('/').map(ToOwned::to_owned).collect::<Vec<_>>()
    } else if let Some(tag) = &app.filter.tag {
        vec![format!("#{tag}")]
    } else {
        vec!["All snippets".to_owned()]
    };
    let full_width = 1 + segments
        .iter()
        .map(|value| 3 + value.chars().count())
        .sum::<usize>();
    if full_width > width {
        let last = segments.pop().unwrap_or_default();
        if segments.is_empty() {
            segments = vec![widgets::truncate_end(&last, width.saturating_sub(4))];
        } else {
            segments = vec![
                "…".to_owned(),
                widgets::truncate_end(&last, width.saturating_sub(8)),
            ];
        }
    }
    let mut spans = vec![Span::styled("~", base.fg(app.theme.muted))];
    let last = segments.len().saturating_sub(1);
    for (index, segment) in segments.into_iter().enumerate() {
        spans.push(Span::styled(" › ", base.fg(app.theme.rule)));
        let style = if index == last {
            if segment.starts_with('#') {
                base.fg(app.theme.tag).add_modifier(Modifier::BOLD)
            } else if segment == "All snippets" {
                base.fg(app.theme.muted)
            } else {
                base.fg(app.theme.pill_primary).add_modifier(Modifier::BOLD)
            }
        } else {
            base
        };
        spans.push(Span::styled(segment, style));
    }
    spans
}
