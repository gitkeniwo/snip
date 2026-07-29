use crate::tui::app::types::App;
use crate::tui::modal::PickerItem;

impl App {
    /// Destination rows for every folder picker: the library root shown under the same
    /// `Uncategorized` label the CLI prints, then each folder path.
    pub(super) fn folder_picker_items(&self) -> Vec<PickerItem> {
        let mut items = vec![PickerItem::new(crate::domain::UNCATEGORIZED, "")];
        items.extend(self.catalog.folders.iter().map(PickerItem::plain));
        items
    }

    pub(super) fn language_picker_items(&self) -> Vec<PickerItem> {
        crate::language::all()
            .iter()
            .map(|language| {
                PickerItem::with_keywords(
                    language.canonical_name,
                    language.aliases[0],
                    language.aliases.iter().copied().chain(language.extension),
                )
            })
            .collect()
    }
}
