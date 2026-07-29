mod confirm;
mod input;
mod picker;
mod render;

use uuid::Uuid;

use super::editor::EditRequest;

pub use confirm::ConfirmModal;
pub use input::{InputModal, TextInput};
pub use picker::{PickerItem, PickerModal};
pub use render::draw_modal;

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
        picker.set_filter("c");
        let filtered = picker.filtered();
        assert_eq!(filtered[0].label, "C");
        assert_eq!(filtered[1].label, "JavaScript");
    }

    #[test]
    fn picker_searches_keywords_and_only_offers_custom_values_without_an_exact_match() {
        let mut picker = language_picker();
        picker.set_filter("ts");
        let filtered = picker.filtered();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].value, "typescript");

        picker.set_filter("my-dsl");
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
        picker.set_filter("script");
        assert_eq!(picker.title(), "Language (javascript) · 2 matches");
    }

    #[test]
    fn replacing_items_rebuilds_the_filtered_cache() {
        let mut picker = language_picker();
        picker.set_filter("python");
        picker.replace_items(vec![PickerItem::new("Python", "python")]);
        assert_eq!(picker.filtered().len(), 1);
        assert_eq!(picker.filtered()[0].value, "python");
    }
}
