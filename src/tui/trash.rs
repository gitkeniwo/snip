use std::sync::Arc;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

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
    /// The selected entry's package, loaded so the preview pane can render it.
    /// Cached because loading it per frame would re-read the package from disk.
    pub preview: Option<Arc<crate::domain::Snippet>>,
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

/// The trash list occupies the snippet pane rather than a popup, so the sidebar
/// stays reachable and the preview pane can show what is about to be restored.
pub fn draw_trash_list(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
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
            .block(widgets::pane_block_tinted(
                &format!("Trash ({})", app.trash.entries.len()),
                true,
                app.theme,
                app.theme.accent_alt,
            ))
            .highlight_symbol("▌ ")
            .highlight_style(app.theme.selected()),
        area,
        &mut state,
    );
}
