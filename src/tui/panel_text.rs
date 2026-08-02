use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::selection::text_width;
use super::theme::TuiTheme;
use super::widgets;

pub fn title_line(name: &str, theme: TuiTheme) -> Line<'static> {
    Line::from(Span::styled(
        name.to_owned(),
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    ))
    .centered()
}

pub fn section(label: &str, width: usize, theme: TuiTheme) -> Line<'static> {
    let prefix = "── ";
    let suffix_width =
        width.saturating_sub(text_width(prefix) as usize + text_width(label) as usize + 1);
    Line::from(vec![
        Span::styled(prefix, Style::default().fg(theme.rule)),
        Span::styled(
            label.to_owned(),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", "─".repeat(suffix_width)),
            Style::default().fg(theme.rule),
        ),
    ])
}

pub fn key_value(key: &str, value: String, width: usize, theme: TuiTheme) -> Line<'static> {
    let key = format!("  {key:<12}");
    let value = widgets::truncate_end(&value, width.saturating_sub(key.len()));
    Line::from(vec![
        Span::styled(key, Style::default().fg(theme.muted)),
        Span::styled(value, Style::default().fg(theme.bar_fg)),
    ])
}
