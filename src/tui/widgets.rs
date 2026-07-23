use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use super::icons;
use super::theme::TuiTheme;
use crate::domain::Snippet;

pub const PILL_OPEN: &str = "\u{e0b6}";
pub const PILL_CLOSE: &str = "\u{e0b4}";

pub fn pane_block(title: &str, focused: bool, theme: TuiTheme) -> Block<'static> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            if focused {
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.muted)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { theme.accent } else { theme.border }))
}

pub fn preview_block(
    focused: bool,
    theme: TuiTheme,
    snippet: Option<&Snippet>,
    fragment_index: usize,
    width: u16,
) -> Block<'static> {
    let mut block = pane_block("Preview", focused, theme);
    let Some(fragment) = snippet.and_then(|snippet| snippet.loaded_fragments.get(fragment_index))
    else {
        return block;
    };

    let language = icons::language_name(&fragment.language);
    let line_count = fragment.content.lines().count();
    let count = format!(
        "{line_count} line{}",
        if line_count == 1 { "" } else { "s" }
    );
    block = block.title_bottom(
        Line::from(Span::styled(
            format!(" {language} "),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ))
        .left_aligned(),
    );

    let available = width.saturating_sub(2) as usize;
    if language.chars().count() + count.chars().count() + 4 < available {
        block = block.title_bottom(
            Line::from(vec![
                Span::styled(
                    format!(" {line_count}"),
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" line{} ", if line_count == 1 { "" } else { "s" }),
                    Style::default().fg(theme.muted),
                ),
            ])
            .right_aligned(),
        );
    }
    block
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    area
}

pub fn inset_left(area: Rect, amount: u16) -> Rect {
    let amount = amount.min(area.width);
    Rect {
        x: area.x.saturating_add(amount),
        width: area.width.saturating_sub(amount),
        ..area
    }
}

pub fn draw_rule(frame: &mut Frame<'_>, area: Rect, theme: TuiTheme) {
    frame.render_widget(
        Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(theme.rule)),
        area,
    );
}

pub fn truncate_end(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    value
        .chars()
        .take(width.saturating_sub(1))
        .chain(std::iter::once('…'))
        .collect()
}

pub fn pill_cap(
    symbol: &'static str,
    fill: ratatui::style::Color,
    surround: ratatui::style::Color,
) -> Span<'static> {
    Span::styled(symbol, Style::default().fg(fill).bg(surround))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn powerline_caps_keep_the_outer_pill_direction_and_colors() {
        let open = pill_cap(PILL_OPEN, Color::Cyan, Color::Black);
        let close = pill_cap(PILL_CLOSE, Color::Cyan, Color::Black);
        assert_eq!(open.content, PILL_OPEN);
        assert_eq!(close.content, PILL_CLOSE);
        assert_eq!(open.style.fg, Some(Color::Cyan));
        assert_eq!(open.style.bg, Some(Color::Black));
        assert_eq!(close.style.fg, Some(Color::Cyan));
        assert_eq!(close.style.bg, Some(Color::Black));
    }

    #[test]
    fn truncation_preserves_unicode_characters_and_adds_an_ellipsis() {
        assert_eq!(truncate_end("你好 Rust", 5), "你好 R…");
        assert_eq!(truncate_end("short", 8), "short");
    }
}
