use super::ModalAction;

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

fn char_byte_index(value: &str, character: usize) -> usize {
    value
        .char_indices()
        .nth(character)
        .map_or(value.len(), |(index, _)| index)
}
