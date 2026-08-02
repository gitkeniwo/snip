use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};

use super::super::app::App;
use super::super::widgets;
use super::text::gist_panel_text;

pub fn draw_gist(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let popup_width = 64.min(area.width);
    // Two border cells plus four cells of padding on each side.
    let content_width = popup_width.saturating_sub(10) as usize;
    let text = gist_panel_text(app, content_width);
    let content_height = u16::try_from(text.lines.len()).unwrap_or(u16::MAX);
    let popup = widgets::centered_rect(popup_width, content_height.saturating_add(2), area);
    frame.render_widget(Clear, popup);
    super::super::widgets::fill_surface(frame, popup, app.theme);
    let block = Block::default()
        .title(Line::from(" Gist ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(app.theme.accent))
        .padding(Padding::new(4, 4, 0, 0));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    frame.render_widget(Paragraph::new(text).alignment(Alignment::Left), inner);
}
