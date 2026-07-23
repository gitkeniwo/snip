use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap};
use uuid::Uuid;

use super::editor::EditRequest;
use super::theme::TuiTheme;
use super::widgets;

#[derive(Clone, Debug)]
pub enum Modal {
    Input(InputModal),
    Confirm(ConfirmModal),
    Picker(PickerModal),
}

impl Modal {
    pub fn action(&self) -> &ModalAction {
        match self {
            Self::Input(modal) => &modal.action,
            Self::Confirm(modal) => &modal.action,
            Self::Picker(modal) => &modal.action,
        }
    }

    pub fn set_error(&mut self, error: impl Into<String>) {
        let error = Some(error.into());
        match self {
            Self::Input(modal) => modal.error = error,
            Self::Confirm(modal) => modal.error = error,
            Self::Picker(modal) => modal.error = error,
        }
    }
}

#[derive(Clone, Debug)]
pub struct InputModal {
    pub label: String,
    pub value: String,
    pub cursor: usize,
    pub action: ModalAction,
    pub error: Option<String>,
}

impl InputModal {
    pub fn new(label: impl Into<String>, value: impl Into<String>, action: ModalAction) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self {
            label: label.into(),
            value,
            cursor,
            action,
            error: None,
        }
    }

    pub fn insert(&mut self, value: char) {
        let byte = char_byte_index(&self.value, self.cursor);
        self.value.insert(byte, value);
        self.cursor += 1;
        self.error = None;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = char_byte_index(&self.value, self.cursor);
        let start = char_byte_index(&self.value, self.cursor - 1);
        self.value.replace_range(start..end, "");
        self.cursor -= 1;
        self.error = None;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let start = char_byte_index(&self.value, self.cursor);
        let end = char_byte_index(&self.value, self.cursor + 1);
        self.value.replace_range(start..end, "");
        self.error = None;
    }
}

#[derive(Clone, Debug)]
pub struct ConfirmModal {
    pub title: String,
    pub message: String,
    pub action: ModalAction,
    pub destructive: bool,
    pub error: Option<String>,
}

impl ConfirmModal {
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        action: ModalAction,
        destructive: bool,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            action,
            destructive,
            error: None,
        }
    }
}

/// A picker row. `label` is what the user reads and filters on; `value` is what the
/// action receives. They differ for the library root, shown as `Uncategorized` but
/// submitted as an empty folder path — which also keeps a real folder of that name
/// from colliding with the root entry.
#[derive(Clone, Debug)]
pub struct PickerItem {
    pub label: String,
    pub value: String,
}

impl PickerItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }

    /// A row whose label and value are the same, such as a folder path.
    pub fn plain(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: value.clone(),
            value,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PickerModal {
    pub label: String,
    pub items: Vec<PickerItem>,
    pub filter: String,
    pub selected: usize,
    pub action: ModalAction,
    pub error: Option<String>,
}

impl PickerModal {
    pub fn new(label: impl Into<String>, items: Vec<PickerItem>, action: ModalAction) -> Self {
        Self {
            label: label.into(),
            items,
            filter: String::new(),
            selected: 0,
            action,
            error: None,
        }
    }

    pub fn filtered(&self) -> Vec<&PickerItem> {
        let query = self.filter.to_lowercase();
        self.items
            .iter()
            .filter(|item| query.is_empty() || item.label.to_lowercase().contains(&query))
            .collect()
    }

    pub fn selected_value(&self) -> Option<String> {
        self.filtered()
            .get(self.selected)
            .map(|item| item.value.clone())
    }

    pub fn clamp(&mut self) {
        let len = self.filtered().len();
        self.selected = self.selected.min(len.saturating_sub(1));
    }
}

#[derive(Clone, Debug)]
pub enum ModalAction {
    RenameSnippet { id: Uuid },
    MoveSnippet { id: Uuid },
    EditTags { id: Uuid },
    DeleteSnippet { id: Uuid },
    ForceEdit(EditRequest),
    CreateTitle,
    CreateFolder { title: String },
    CreateLanguage { title: String, folder: String },
    CreateFolderUnder { parent: String },
    RenameFolder { path: String },
    MoveFolder { path: String },
    DeleteFolder { path: String },
    RenameTag { tag: String },
    DeleteTag { tag: String },
    PurgeSnippet { entry_id: String },
}

fn char_byte_index(value: &str, character: usize) -> usize {
    value
        .char_indices()
        .nth(character)
        .map_or(value.len(), |(index, _)| index)
}

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
                            .title(format!(" {} ", picker.label))
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
