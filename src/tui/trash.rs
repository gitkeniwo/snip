use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem};

use crate::error::Result;
use crate::filesystem::Library;
use crate::service::{TrashEntry, trash_entries};

use super::app::App;
use super::widgets;

#[derive(Clone, Debug, Default)]
pub struct TrashState {
    pub open: bool,
    pub entries: Vec<TrashEntry>,
    pub selected: usize,
}

impl TrashState {
    pub fn open(&mut self, library: &Library) -> Result<()> {
        self.open = true;
        self.reload(library)
    }

    pub fn reload(&mut self, library: &Library) -> Result<()> {
        self.entries = trash_entries(library)?;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        Ok(())
    }

    pub fn selected(&self) -> Option<&TrashEntry> {
        self.entries.get(self.selected)
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected as isize + delta)
            .clamp(0, self.entries.len().saturating_sub(1) as isize)
            as usize;
    }
}

pub fn draw_trash(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let popup = widgets::centered_rect(76, 20, area);
    frame.render_widget(Clear, popup);
    let items = app
        .trash
        .entries
        .iter()
        .map(|entry| {
            ListItem::new(vec![
                Line::from(Span::styled(
                    entry.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(vec![
                    Span::styled(
                        entry.original_path.clone(),
                        Style::default().fg(app.theme.muted),
                    ),
                    Span::styled("  ·  ", Style::default().fg(app.theme.rule)),
                    Span::styled(
                        entry.deleted_at.clone(),
                        Style::default().fg(app.theme.muted),
                    ),
                ]),
            ])
        })
        .collect::<Vec<_>>();
    let mut state = ratatui::widgets::ListState::default();
    state.select((!items.is_empty()).then_some(app.trash.selected));
    frame.render_stateful_widget(
        List::new(items)
            .block(
                Block::default()
                    .title(format!(" Trash ({}) ", app.trash.entries.len()))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(app.theme.accent_alt)),
            )
            .highlight_symbol("▌ ")
            .highlight_style(app.theme.selected()),
        popup,
        &mut state,
    );
}
