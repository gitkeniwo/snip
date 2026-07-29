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

#[derive(Clone, Debug, Default)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn insert(&mut self, value: char) {
        let byte = char_byte_index(&self.value, self.cursor);
        self.value.insert(byte, value);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let end = char_byte_index(&self.value, self.cursor);
        let start = char_byte_index(&self.value, self.cursor - 1);
        self.value.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let start = char_byte_index(&self.value, self.cursor);
        let end = char_byte_index(&self.value, self.cursor + 1);
        self.value.replace_range(start..end, "");
    }
}

#[derive(Clone, Debug)]
pub struct InputModal {
    pub label: String,
    pub input: TextInput,
    pub action: ModalAction,
    pub error: Option<String>,
}

impl InputModal {
    pub fn new(label: impl Into<String>, value: impl Into<String>, action: ModalAction) -> Self {
        Self {
            label: label.into(),
            input: TextInput::new(value),
            action,
            error: None,
        }
    }

    pub fn insert(&mut self, value: char) {
        self.input.insert(value);
        self.error = None;
    }

    pub fn backspace(&mut self) {
        self.input.backspace();
        self.error = None;
    }

    pub fn delete(&mut self) {
        self.input.delete();
        self.error = None;
    }
}

impl std::ops::Deref for InputModal {
    type Target = TextInput;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl std::ops::DerefMut for InputModal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.input
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
    pub keywords: Vec<String>,
    custom: bool,
}

impl PickerItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            keywords: Vec::new(),
            custom: false,
        }
    }

    pub fn with_keywords(
        label: impl Into<String>,
        value: impl Into<String>,
        keywords: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            keywords: keywords.into_iter().map(Into::into).collect(),
            custom: false,
        }
    }

    /// A row whose label and value are the same, such as a folder path.
    pub fn plain(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: value.clone(),
            value,
            keywords: Vec::new(),
            custom: false,
        }
    }

    fn custom(value: &str) -> Self {
        Self {
            label: format!("use “{value}”"),
            value: value.to_owned(),
            keywords: Vec::new(),
            custom: true,
        }
    }

    fn match_rank(&self, query: &str) -> Option<u8> {
        let label = self.label.to_lowercase();
        let value = self.value.to_lowercase();
        let keywords = self
            .keywords
            .iter()
            .map(|keyword| keyword.to_lowercase())
            .collect::<Vec<_>>();
        if label == query || value == query || keywords.iter().any(|keyword| keyword == query) {
            Some(0)
        } else if label.starts_with(query) || value.starts_with(query) {
            Some(1)
        } else if keywords.iter().any(|keyword| keyword.starts_with(query)) {
            Some(2)
        } else if label.contains(query) || value.contains(query) {
            Some(3)
        } else if keywords.iter().any(|keyword| keyword.contains(query)) {
            Some(4)
        } else {
            None
        }
    }

    fn has_exact_match(&self, query: &str) -> bool {
        self.label.eq_ignore_ascii_case(query)
            || self.value.eq_ignore_ascii_case(query)
            || self
                .keywords
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case(query))
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
    pub allow_custom: bool,
    pub current_value: Option<String>,
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
            allow_custom: false,
            current_value: None,
        }
    }

    pub fn allow_custom(mut self) -> Self {
        self.allow_custom = true;
        self
    }

    pub fn with_current_value(mut self, value: impl Into<String>) -> Self {
        self.current_value = Some(value.into());
        self
    }

    pub fn select_value(&mut self, value: &str) {
        self.selected = self
            .items
            .iter()
            .position(|item| item.value.eq_ignore_ascii_case(value))
            .unwrap_or(0);
    }

    pub fn filtered(&self) -> Vec<PickerItem> {
        let query = self.filter.trim().to_lowercase();
        if query.is_empty() {
            return self.items.clone();
        }
        let exact_match = self.items.iter().any(|item| item.has_exact_match(&query));
        let mut matches = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                item.match_rank(&query)
                    .map(|rank| (rank, index, item.clone()))
            })
            .collect::<Vec<_>>();
        matches.sort_by_key(|(rank, index, _)| (*rank, *index));
        let mut items = matches
            .into_iter()
            .map(|(_, _, item)| item)
            .collect::<Vec<_>>();
        if self.allow_custom && !exact_match {
            items.push(PickerItem::custom(self.filter.trim()));
        }
        items
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

    pub fn title(&self) -> String {
        let current = self
            .current_value
            .as_deref()
            .map_or_else(String::new, |value| format!(" ({value})"));
        let matches = self
            .filtered()
            .into_iter()
            .filter(|item| !item.custom)
            .count();
        let direct_use = if self.allow_custom && !self.filter.trim().is_empty() && matches == 0 {
            " · ⏎ direct use"
        } else {
            ""
        };
        format!("{}{current} · {matches} matches{direct_use}", self.label)
    }
}

#[derive(Clone, Debug)]
pub enum ModalAction {
    RenameSnippet { id: Uuid },
    MoveSnippet { id: Uuid },
    EditTags { id: Uuid },
    EditLanguage { id: Uuid, fragment_index: usize },
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
    GitCommit,
    GitAutoCommitInterval,
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
                            .title(format!(" {} ", picker.title()))
                            .title_bottom(format!(
                                " /{} ",
                                if picker.filter.is_empty() {
                                    "type to filter"
                                } else {
                                    picker.filter.as_str()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn language_picker() -> PickerModal {
        PickerModal::new(
            "Language",
            vec![
                PickerItem::with_keywords("JavaScript", "javascript", ["js", "node"]),
                PickerItem::with_keywords("C", "c", ["c"]),
                PickerItem::with_keywords("TypeScript", "typescript", ["ts"]),
            ],
            ModalAction::CreateTitle,
        )
        .allow_custom()
        .with_current_value("javascript")
    }

    #[test]
    fn picker_ranks_exact_primary_matches_before_substrings() {
        let mut picker = language_picker();
        picker.filter = "c".to_owned();
        let filtered = picker.filtered();
        assert_eq!(filtered[0].label, "C");
        assert_eq!(filtered[1].label, "JavaScript");
    }

    #[test]
    fn picker_searches_keywords_and_only_offers_custom_values_without_an_exact_match() {
        let mut picker = language_picker();
        picker.filter = "ts".to_owned();
        let filtered = picker.filtered();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].value, "typescript");

        picker.filter = "my-dsl".to_owned();
        let filtered = picker.filtered();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "use “my-dsl”");
        assert_eq!(filtered[0].value, "my-dsl");
        assert_eq!(
            picker.title(),
            "Language (javascript) · 0 matches · ⏎ direct use"
        );
    }

    #[test]
    fn picker_title_reports_current_value_and_real_match_count() {
        let mut picker = language_picker();
        picker.filter = "script".to_owned();
        assert_eq!(picker.title(), "Language (javascript) · 2 matches");
    }
}
