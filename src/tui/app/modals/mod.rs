mod actions;
mod mutation;
mod openers;
mod pickers;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::types::{App, Effect};
use crate::tui::modal::{Modal, ModalAction};
use crate::tui::state::StatusLevel;

impl App {
    pub(super) fn handle_modal_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let mut submit = false;
        let mut cancel = false;
        if let Some(modal) = self.modal.as_mut() {
            match modal {
                Modal::Input(input) => match key.code {
                    KeyCode::Enter => submit = true,
                    KeyCode::Esc => cancel = true,
                    KeyCode::Left => input.cursor = input.cursor.saturating_sub(1),
                    KeyCode::Right => {
                        input.cursor = (input.cursor + 1).min(input.value.chars().count())
                    }
                    KeyCode::Home => input.cursor = 0,
                    KeyCode::End => input.cursor = input.value.chars().count(),
                    KeyCode::Backspace => input.backspace(),
                    KeyCode::Delete => input.delete(),
                    KeyCode::Char(value)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        input.insert(value)
                    }
                    _ => {}
                },
                Modal::Confirm(_) => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => submit = true,
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => cancel = true,
                    _ => {}
                },
                Modal::Picker(picker) => match key.code {
                    KeyCode::Enter => submit = true,
                    KeyCode::Esc => cancel = true,
                    // The filter is a text field, so `j`/`k` must stay typable: folder
                    // names like "Docker" would otherwise be unreachable. Navigate with
                    // the arrows or Ctrl-n/Ctrl-p instead.
                    KeyCode::Down => {
                        picker.selected = picker
                            .selected
                            .saturating_add(1)
                            .min(picker.filtered().len().saturating_sub(1));
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        picker.selected = picker
                            .selected
                            .saturating_add(1)
                            .min(picker.filtered().len().saturating_sub(1));
                    }
                    KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        picker.selected = picker.selected.saturating_sub(1)
                    }
                    KeyCode::Home => picker.selected = 0,
                    KeyCode::End => picker.selected = picker.filtered().len().saturating_sub(1),
                    KeyCode::Backspace => {
                        picker.pop_filter();
                        picker.selected = 0;
                        picker.error = None;
                    }
                    KeyCode::Char(value)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        picker.push_filter(value);
                        picker.selected = 0;
                        picker.error = None;
                    }
                    _ => {}
                },
            }
        }
        if cancel {
            let force_edit = self
                .modal
                .as_ref()
                .is_some_and(|modal| matches!(modal.action(), ModalAction::ForceEdit(_)));
            self.modal = None;
            if force_edit {
                self.set_status("edited content discarded", StatusLevel::Info);
            }
            return Vec::new();
        }
        if submit {
            return self.submit_modal();
        }
        Vec::new()
    }

    pub(super) fn submit_modal(&mut self) -> Vec<Effect> {
        let Some(mut modal) = self.modal.take() else {
            return Vec::new();
        };
        let action = modal.action().clone();
        let value = match &modal {
            Modal::Input(input) => Some(input.value.clone()),
            Modal::Confirm(_) => None,
            Modal::Picker(picker) => picker.selected_value(),
        };
        if matches!(modal, Modal::Picker(_)) && value.is_none() {
            modal.set_error("no matching item");
            self.modal = Some(modal);
            return Vec::new();
        }
        match self.perform_modal_action(action, value.as_deref()) {
            Ok((effects, message)) => {
                if !message.is_empty() {
                    self.set_status(message, StatusLevel::Info);
                }
                effects
            }
            Err(error) => {
                modal.set_error(error.to_string());
                self.modal = Some(modal);
                Vec::new()
            }
        }
    }
}
