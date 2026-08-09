use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::app::App;
use super::git_panel;
use super::state::sort_indicator;
use super::theme::TuiTheme;
use super::widgets;

pub fn draw_top_bar(frame: &mut Frame<'_>, app: &App, area: Rect) {
    frame.render_widget(
        Block::default().style(Style::default().fg(app.theme.bar_fg).bg(app.theme.bar_bg)),
        area,
    );
    let counts = if let Some(index) = app.list_state.selected() {
        format!("#{}/{}", index + 1, app.visible.len())
    } else {
        format!("0/{}", app.visible.len())
    };
    let brand_color = if app.modal.is_some() {
        app.theme.accent_alt
    } else if !app.search.query.is_empty() {
        app.theme.warning
    } else {
        app.theme.pill_primary
    };
    let git = (area.width >= 60)
        .then(|| git_panel::badge(&app.git, app.icon_mode, app.theme))
        .flatten();
    let right = widgets::square_end(top_position_pill(
        git,
        sort_indicator(app.sort),
        &counts,
        app.theme,
    ));
    let right_width = right.width().min(area.width as usize) as u16;
    let regions = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(2),
        Constraint::Length(right_width),
    ])
    .split(area);
    let left = widgets::square_start(top_context_pill(
        app,
        regions[0].width as usize,
        brand_color,
    ));
    frame.render_widget(Paragraph::new(left), regions[0]);
    frame.render_widget(
        Paragraph::new(right).alignment(Alignment::Right),
        regions[2],
    );
}

fn top_context_pill(app: &App, width: usize, primary: ratatui::style::Color) -> Line<'static> {
    let primary_style = Style::default()
        .fg(app.theme.legible_on(primary, app.theme.selection_fg))
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
    let secondary_style = Style::default()
        .fg(app.theme.legible_on(secondary, app.theme.bar_fg))
        .bg(secondary);
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

fn top_position_pill(
    git: Option<(String, ratatui::style::Color)>,
    sort: Option<&str>,
    counts: &str,
    theme: TuiTheme,
) -> Line<'static> {
    // Right-aligned cluster: each sub-pill opens with a rounded cap facing
    // its own content (`(`), meets the previous pill with a flat edge, and
    // the outer right edge is squared flush against the screen corner.
    let primary = theme.pill_primary;
    let secondary = theme.pill_secondary;
    let mut spans = Vec::new();
    if let Some((git, color)) = git {
        spans.push(widgets::pill_cap(
            widgets::PILL_OPEN,
            secondary,
            theme.bar_bg,
        ));
        spans.push(Span::styled(
            format!(" {git} "),
            Style::default()
                .fg(theme.legible_on(secondary, color))
                .bg(secondary)
                .add_modifier(Modifier::BOLD),
        ));
        if sort.is_some() {
            spans.push(Span::styled(" ", Style::default().bg(secondary)));
        } else {
            spans.push(widgets::pill_cap(widgets::PILL_OPEN, primary, secondary));
        }
    }
    if let Some(sort) = sort {
        if spans.is_empty() {
            spans.push(widgets::pill_cap(
                widgets::PILL_OPEN,
                secondary,
                theme.bar_bg,
            ));
        }
        spans.push(Span::styled(
            format!(" {sort} "),
            Style::default()
                .fg(theme.legible_on(secondary, theme.bar_fg))
                .bg(secondary)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(widgets::pill_cap(widgets::PILL_OPEN, primary, secondary));
    } else if spans.is_empty() {
        spans.push(widgets::pill_cap(widgets::PILL_OPEN, primary, theme.bar_bg));
    }
    spans.push(Span::styled(
        format!(" {counts} "),
        Style::default()
            .fg(theme.legible_on(primary, theme.selection_fg))
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
    let mut segments = if app.filter.uncategorized {
        vec!["Uncategorized".to_owned()]
    } else if let Some(folder) = &app.filter.folder {
        folder.split('/').map(ToOwned::to_owned).collect::<Vec<_>>()
    } else if let Some(tag) = &app.filter.tag {
        vec![format!("#{tag}")]
    } else {
        vec!["All snippets".to_owned()]
    };
    if !app.search.query.is_empty() {
        segments.push(format!("/{}", app.search.query));
    }
    let root = app.library.display_name();
    let root_len = root.chars().count();
    let full_width = root_len
        + segments
            .iter()
            .map(|value| 3 + value.chars().count())
            .sum::<usize>();
    if full_width > width {
        let last = segments.pop().unwrap_or_default();
        if segments.is_empty() {
            segments = vec![widgets::truncate_end(
                &last,
                width.saturating_sub(root_len + 3),
            )];
        } else {
            segments = vec![
                "…".to_owned(),
                widgets::truncate_end(&last, width.saturating_sub(root_len + 8)),
            ];
        }
    }
    let secondary = app.theme.pill_secondary;
    let mut spans = vec![Span::styled(
        root.to_owned(),
        base.fg(app.theme.legible_on(secondary, app.theme.muted)),
    )];
    let last = segments.len().saturating_sub(1);
    for (index, segment) in segments.into_iter().enumerate() {
        spans.push(Span::styled(
            " › ",
            base.fg(app.theme.legible_on(secondary, app.theme.rule)),
        ));
        let style = if index == last {
            if segment.starts_with('/') {
                base.fg(app.theme.legible_on(secondary, app.theme.warning))
                    .add_modifier(Modifier::BOLD)
            } else if segment.starts_with('#') {
                base.fg(app.theme.legible_on(secondary, app.theme.tag))
                    .add_modifier(Modifier::BOLD)
            } else if segment == "All snippets" {
                base.fg(app.theme.legible_on(secondary, app.theme.muted))
            } else {
                base.fg(app.theme.legible_on(secondary, app.theme.pill_primary))
                    .add_modifier(Modifier::BOLD)
            }
        } else {
            base
        };
        spans.push(Span::styled(segment, style));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_label_prefers_neutral_bar_foreground() {
        let theme = TuiTheme::default_for(crate::theme::Appearance::Light);
        let line = top_position_pill(None, Some("↓ modified"), "#1/2", theme);
        let sort = line
            .spans
            .iter()
            .find(|span| span.content == " ↓ modified ")
            .unwrap();

        assert_eq!(sort.style.fg, Some(theme.bar_fg));
        assert_eq!(sort.style.bg, Some(theme.pill_secondary));
    }
}
