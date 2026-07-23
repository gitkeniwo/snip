use crate::error::Result;
use crate::filesystem::Library;
use crate::service::{TrashEntry, trash_entries};

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
