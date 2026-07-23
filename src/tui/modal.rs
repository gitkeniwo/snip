use uuid::Uuid;

use super::editor::EditRequest;

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

#[derive(Clone, Debug)]
pub struct PickerModal {
    pub label: String,
    pub items: Vec<String>,
    pub filter: String,
    pub selected: usize,
    pub action: ModalAction,
    pub error: Option<String>,
}

impl PickerModal {
    pub fn new(label: impl Into<String>, items: Vec<String>, action: ModalAction) -> Self {
        Self {
            label: label.into(),
            items,
            filter: String::new(),
            selected: 0,
            action,
            error: None,
        }
    }

    pub fn filtered(&self) -> Vec<&str> {
        let query = self.filter.to_lowercase();
        self.items
            .iter()
            .filter(|item| query.is_empty() || item.to_lowercase().contains(&query))
            .map(String::as_str)
            .collect()
    }

    pub fn selected_value(&self) -> Option<String> {
        self.filtered()
            .get(self.selected)
            .map(|value| (*value).to_owned())
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
