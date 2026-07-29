use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap};

use super::Modal;
use crate::tui::theme::TuiTheme;
use crate::tui::widgets;

pub fn draw_modal(frame: &mut Frame<'_>, area: Rect, modal: &mut Modal, theme: TuiTheme) {
    match modal {
        Modal::Input(_) => {}
        Modal::Confirm(confirm) => {
            let popup = widgets::centered_rect(62, 8, area);
            frame.render_widget(Clear, popup);
            let border = if confirm.destructive {
                theme.error
            } else {
                theme.accent
            };
            let mut lines = vec![Line::from(confirm.message.clone()), Line::default()];
            if let Some(error) = &confirm.error {
                lines.push(Line::from(Span::styled(
                    error.clone(),
                    Style::default().fg(theme.error),
                )));
            }
            frame.render_widget(
                Paragraph::new(lines)
                    .block(
                        Block::default()
                            .title(format!(" {} ", confirm.title))
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(border)),
                    )
                    .wrap(Wrap { trim: false }),
                popup,
            );
        }
        Modal::Picker(picker) => {
            let popup = widgets::centered_rect(62, 18, area);
            frame.render_widget(Clear, popup);
            let filtered = picker.filtered();
            let items = filtered
                .iter()
                .map(|item| ListItem::new(item.label.clone()))
                .collect::<Vec<_>>();
            let mut state = ratatui::widgets::ListState::default();
            state.select((!items.is_empty()).then_some(picker.selected));
            frame.render_stateful_widget(
                List::new(items)
                    .block(
                        Block::default()
                            .title(format!(" {} ", picker.title()))
                            .title_bottom(format!(
                                " /{} ",
                                if picker.filter().is_empty() {
                                    "type to filter"
                                } else {
                                    picker.filter()
                                }
                            ))
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(theme.accent)),
                    )
                    .highlight_symbol("▌ ")
                    .highlight_style(theme.selected()),
                popup,
                &mut state,
            );
            if let Some(error) = &picker.error {
                let error_area = Rect {
                    x: popup.x.saturating_add(2),
                    y: popup.bottom().saturating_sub(2),
                    width: popup.width.saturating_sub(4),
                    height: 1,
                };
                frame.render_widget(
                    Paragraph::new(error.clone()).style(Style::default().fg(theme.error)),
                    error_area,
                );
            }
        }
    }
}
