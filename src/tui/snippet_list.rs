use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use super::app::App;
use super::icons::snippet_badge;

pub fn items(app: &App) -> Vec<ListItem<'static>> {
    app.visible
        .iter()
        .filter_map(|row| {
            let snippet = app
                .catalog
                .snippets
                .iter()
                .find(|snippet| snippet.id == row.snippet_id)?;
            let mut markers = String::new();
            if snippet.pinned {
                markers.push('★');
            }
            if snippet.locked {
                markers.push('🔒');
            }
            let marker = if markers.is_empty() {
                String::new()
            } else {
                format!("{markers} ")
            };
            let mut lines = vec![Line::from(vec![
                Span::raw(marker),
                Span::styled(
                    format!("[{}]", snippet_badge(snippet)),
                    Style::default().fg(app.theme.accent_alt),
                ),
                Span::raw(" "),
                Span::styled(
                    snippet.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ])];
            if !snippet.folder.is_empty() {
                lines.push(Line::from(Span::styled(
                    snippet.folder.clone(),
                    Style::default().fg(app.theme.muted),
                )));
            }
            if let Some(excerpt) = &row.excerpt
                && excerpt != &snippet.title
            {
                lines.push(Line::from(Span::styled(
                    excerpt.clone(),
                    Style::default().fg(app.theme.muted),
                )));
            }
            Some(ListItem::new(lines))
        })
        .collect()
}
