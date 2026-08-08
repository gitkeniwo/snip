use std::time::{Duration, Instant};

use super::super::super::layout::{contains, inner};
use super::super::super::preview::{PreviewTarget, has_readme};
use super::super::super::state::{Pane, SidebarItem, StatusLevel};
use super::super::types::App;

impl App {
    pub(in super::super) fn navigate_down(&mut self) {
        let fragment_count = self
            .selected_snippet()
            .map_or(0, |snippet| snippet.loaded_fragments.len());
        if let Some(grab) = self.fragment_grab.as_mut() {
            grab.current = grab
                .current
                .saturating_add(1)
                .min(fragment_count.saturating_sub(1));
        } else if self.show_help {
            self.help_scroll = self.help_scroll.saturating_add(1);
        } else if self.trash.open && self.focus == Pane::List {
            self.trash.move_selection(1);
            self.sync_trash_preview();
        } else {
            match self.focus {
                Pane::Sidebar => self.move_sidebar(1),
                Pane::List => self.move_list(1),
                Pane::Preview => self.preview_scroll = self.preview_scroll.saturating_add(1),
            }
        }
    }

    pub(in super::super) fn navigate_up(&mut self) {
        if let Some(grab) = self.fragment_grab.as_mut() {
            grab.current = grab.current.saturating_sub(1);
        } else if self.show_help {
            self.help_scroll = self.help_scroll.saturating_sub(1);
        } else if self.trash.open && self.focus == Pane::List {
            self.trash.move_selection(-1);
            self.sync_trash_preview();
        } else {
            match self.focus {
                Pane::Sidebar => self.move_sidebar(-1),
                Pane::List => self.move_list(-1),
                Pane::Preview => self.preview_scroll = self.preview_scroll.saturating_sub(1),
            }
        }
    }

    pub(in super::super) fn navigate_first(&mut self) {
        if self.trash.open && self.focus == Pane::List {
            self.trash.selected = 0;
            self.sync_trash_preview();
        } else {
            match self.focus {
                Pane::Sidebar => self.select_sidebar(0),
                Pane::List => self.select_list(0),
                Pane::Preview => self.preview_scroll = 0,
            }
        }
    }

    pub(in super::super) fn navigate_last(&mut self) {
        if self.trash.open && self.focus == Pane::List {
            self.trash.selected = self.trash.entries.len().saturating_sub(1);
            self.sync_trash_preview();
        } else {
            match self.focus {
                Pane::Sidebar => self.select_sidebar(self.sidebar.rows.len().saturating_sub(1)),
                Pane::List => self.select_list(self.visible.len().saturating_sub(1)),
                Pane::Preview => self.preview_scroll = u16::MAX,
            }
        }
    }

    pub(in super::super) fn navigate_page_down(&mut self) {
        if self.show_help {
            self.help_scroll = self.help_scroll.saturating_add(10);
        } else if self.trash.open && self.focus == Pane::List {
            self.trash.move_selection(10);
            self.sync_trash_preview();
        } else {
            match self.focus {
                Pane::Sidebar => self.move_sidebar(10),
                Pane::List => self.move_list(10),
                Pane::Preview => self.preview_scroll = self.preview_scroll.saturating_add(10),
            }
        }
    }

    pub(in super::super) fn navigate_page_up(&mut self) {
        if self.show_help {
            self.help_scroll = self.help_scroll.saturating_sub(10);
        } else if self.trash.open && self.focus == Pane::List {
            self.trash.move_selection(-10);
            self.sync_trash_preview();
        } else {
            match self.focus {
                Pane::Sidebar => self.move_sidebar(-10),
                Pane::List => self.move_list(-10),
                Pane::Preview => self.preview_scroll = self.preview_scroll.saturating_sub(10),
            }
        }
    }

    pub(in super::super) fn drop_grabbed_fragment(&mut self) {
        let Some(grab) = self.fragment_grab else {
            return;
        };
        if grab.current == grab.origin {
            self.fragment_grab = None;
            self.set_status("fragment order unchanged", StatusLevel::Info);
            return;
        }
        let Some(id) = self.selected_snippet().map(|snippet| snippet.id) else {
            self.fragment_grab = None;
            return;
        };
        let result = crate::service::reorder_fragment(
            &self.library,
            &id.to_string(),
            &(grab.origin + 1).to_string(),
            grab.current + 1,
            None,
            false,
        );
        match result {
            Ok(_) => match self.rescan() {
                Ok(()) => {
                    self.preview_target = PreviewTarget::Fragment(grab.current);
                    self.fragment_grab = None;
                    self.set_status("fragment moved", StatusLevel::Info);
                }
                Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
            },
            Err(error) => self.set_status(error.to_string(), StatusLevel::Error),
        }
    }

    pub(in super::super) fn drill_back(&mut self) {
        match self.focus {
            Pane::Preview => self.focus = Pane::List,
            Pane::List if self.show_sidebar => self.focus = Pane::Sidebar,
            Pane::List => {}
            Pane::Sidebar => {
                let folder = self.sidebar.selected().and_then(|row| match &row.item {
                    SidebarItem::Folder(folder) if row.expanded => Some(folder.clone()),
                    _ => None,
                });
                if let Some(folder) = folder {
                    self.sidebar.expanded.remove(&folder);
                    self.rebuild_sidebar();
                    self.sync_sidebar_filter();
                }
            }
        }
    }

    pub(in super::super) fn drill_forward(&mut self) {
        match self.focus {
            Pane::Sidebar => self.apply_sidebar_filter(),
            Pane::List => self.focus = Pane::Preview,
            Pane::Preview => {}
        }
    }

    pub(in super::super) fn toggle_line_numbers(&mut self) {
        self.show_line_numbers = !self.show_line_numbers;
        self.preview_selection.clear();
        let label = if self.show_line_numbers {
            "line numbers on"
        } else {
            "line numbers off"
        };
        let mut config = match crate::config::AppConfig::load() {
            Ok(config) => config,
            Err(error) => {
                self.set_status(
                    format!("{label} for this session: {error}"),
                    StatusLevel::Error,
                );
                return;
            }
        };
        config
            .tui
            .get_or_insert_with(crate::config::TuiConfig::default)
            .line_numbers = self.show_line_numbers;
        match config.save() {
            Ok(()) => self.set_status(label, StatusLevel::Info),
            Err(error) => self.set_status(
                format!("{label} for this session: {error}"),
                StatusLevel::Error,
            ),
        }
    }

    pub(in super::super) fn toggle_density(&mut self) {
        self.density = self.density.next();
        let mut config = match crate::config::AppConfig::load() {
            Ok(config) => config,
            Err(error) => {
                self.set_status(
                    format!("density changed for this session: {error}"),
                    StatusLevel::Error,
                );
                return;
            }
        };
        config
            .tui
            .get_or_insert_with(crate::config::TuiConfig::default)
            .density = self.density;
        match config.save() {
            Ok(()) => self.set_status(
                format!("list density: {}", self.density.label()),
                StatusLevel::Info,
            ),
            Err(error) => self.set_status(
                format!("density changed for this session: {error}"),
                StatusLevel::Error,
            ),
        }
    }

    pub(in super::super) fn toggle_sidebar(&mut self) {
        self.show_sidebar = !self.show_sidebar;
        if !self.show_sidebar && self.focus == Pane::Sidebar {
            self.focus = Pane::List;
        }
        self.set_status(
            if self.show_sidebar {
                "library pane shown"
            } else {
                "library pane hidden"
            },
            StatusLevel::Info,
        );
    }

    pub(super) fn click_at(&mut self, column: u16, row: u16) {
        if contains(self.layout.sidebar, column, row) {
            let content = inner(self.layout.sidebar);
            if !contains(content, column, row) {
                self.focus = Pane::Sidebar;
                return;
            }
            let index = self.sidebar.list_state.offset() + (row - content.y) as usize;
            if index >= self.sidebar.rows.len() {
                return;
            }
            self.sidebar.list_state.select(Some(index));
            self.focus = Pane::Sidebar;
            let fold_column = content
                .x
                .saturating_add(self.sidebar.rows[index].depth.saturating_mul(2) as u16);
            if self.sidebar.rows[index].has_children && column <= fold_column.saturating_add(1) {
                self.toggle_sidebar_folder();
            } else {
                // A click is a deliberate act, so it activates the row rather
                // than merely moving the cursor onto it.
                self.activate_sidebar_row();
            }
            return;
        }
        if contains(self.layout.list, column, row) {
            let content = inner(self.layout.list);
            if !contains(content, column, row) {
                self.focus = Pane::List;
                return;
            }
            if self.trash.open {
                // Trash rows are keyboard-driven; a click just takes focus.
                self.focus = Pane::List;
                return;
            }
            let index =
                self.list_state.offset() + ((row - content.y) / self.density.row_height()) as usize;
            if index >= self.visible.len() {
                return;
            }
            self.select_list(index);
            self.focus = Pane::List;
            let now = Instant::now();
            let double = self.last_click.is_some_and(|(previous, at)| {
                previous == index && now.duration_since(at) < Duration::from_millis(500)
            });
            self.last_click = Some((index, now));
            if double {
                self.focus = Pane::Preview;
            }
            return;
        }
        if contains(self.layout.preview_fragments, column, row) {
            if row == self.layout.preview_fragments.y {
                self.toggle_fragments_expanded();
                self.focus = Pane::Preview;
                return;
            }
            if let Some(target) = self
                .layout
                .fragment_rows
                .iter()
                .find_map(|(y, target)| (*y == row).then_some(*target))
            {
                self.select_fragment(target);
                self.focus = Pane::Preview;
                return;
            }
            self.focus = Pane::Preview;
            return;
        }
        if contains(self.layout.preview, column, row) {
            self.focus = Pane::Preview;
        }
    }

    pub(super) fn scroll_at(&mut self, column: u16, row: u16, direction: isize) {
        if contains(self.layout.sidebar, column, row) {
            self.move_sidebar(direction);
        } else if contains(self.layout.list, column, row) {
            self.move_list(direction);
        } else if contains(self.layout.preview, column, row) {
            if direction < 0 {
                self.preview_scroll = self.preview_scroll.saturating_sub(3);
            } else {
                self.preview_scroll = self.preview_scroll.saturating_add(3);
            }
        }
    }

    pub(super) fn move_sidebar(&mut self, delta: isize) {
        let len = self.sidebar.rows.len();
        if len == 0 {
            return;
        }
        let mut index = self.sidebar.list_state.selected().unwrap_or(0);
        loop {
            index = (index as isize + delta).clamp(0, len.saturating_sub(1) as isize) as usize;
            if self.sidebar.rows[index].item != SidebarItem::Header
                || index == 0
                || index + 1 == len
            {
                break;
            }
        }
        self.sidebar.list_state.select(Some(index));
        self.sync_sidebar_filter();
    }

    pub(super) fn select_sidebar(&mut self, mut index: usize) {
        if self
            .sidebar
            .rows
            .get(index)
            .is_some_and(|row| row.item == SidebarItem::Header)
        {
            index = (index + 1).min(self.sidebar.rows.len().saturating_sub(1));
        }
        self.sidebar
            .list_state
            .select((!self.sidebar.rows.is_empty()).then_some(index));
        self.sync_sidebar_filter();
    }

    pub(in super::super) fn apply_sidebar_filter(&mut self) {
        if self.activate_sidebar_row() {
            self.focus = Pane::List;
        }
    }

    /// Enter, or a mouse click. Only `Published` needs this: it is a lens you
    /// flip, not a place you go, so it must not fire from mere navigation.
    /// Everything else is a scope and is already applied by the cursor.
    pub(super) fn activate_sidebar_row(&mut self) -> bool {
        match self.sidebar.selected().map(|row| row.item.clone()) {
            Some(SidebarItem::Published) => {
                self.filter.published = !self.filter.published;
                self.refresh_visible();
                // Stay in the sidebar: a toggle is something you flip back.
                false
            }
            _ => self.sync_sidebar_filter(),
        }
    }

    /// Applies the selected row as a *scope*, on every cursor move. `Trash` is
    /// one of those scopes — moving onto it enters the trash view and moving
    /// off leaves again, so it needs no separate open/close step. `Published`
    /// is excluded because a toggle that fired on hover would flip twice as the
    /// cursor passed over it.
    pub(in super::super) fn sync_sidebar_filter(&mut self) -> bool {
        let item = self.sidebar.selected().map(|row| row.item.clone());
        if !matches!(item, Some(SidebarItem::Trash)) && self.trash.open {
            self.trash.open = false;
        }
        match item {
            Some(SidebarItem::Trash) => {
                if !self.trash.open {
                    self.open_trash();
                }
                return false;
            }
            Some(SidebarItem::All) => {
                self.filter.uncategorized = false;
                self.filter.folder = None;
                self.filter.tag = None;
            }
            Some(SidebarItem::Uncategorized) => {
                self.filter.uncategorized = true;
                self.filter.folder = None;
                self.filter.tag = None;
            }
            Some(SidebarItem::Folder(folder)) => {
                self.filter.uncategorized = false;
                self.filter.folder = Some(folder);
                self.filter.tag = None;
            }
            Some(SidebarItem::Tag(tag)) => {
                self.filter.uncategorized = false;
                self.filter.tag = Some(tag);
                self.filter.folder = None;
            }
            // The toggle, and non-selectable rows: never fired by navigation.
            Some(SidebarItem::Published | SidebarItem::Header) | None => return false,
        }
        self.refresh_visible();
        true
    }

    pub(in super::super) fn toggle_sidebar_folder(&mut self) {
        let folder = self.sidebar.selected().and_then(|row| match &row.item {
            SidebarItem::Folder(folder) if row.has_children => Some(folder.clone()),
            _ => None,
        });
        if let Some(folder) = folder {
            if !self.sidebar.expanded.remove(&folder) {
                self.sidebar.expanded.insert(folder);
            }
            self.rebuild_sidebar();
            self.sync_sidebar_filter();
        } else {
            self.apply_sidebar_filter();
        }
    }

    pub(super) fn move_list(&mut self, delta: isize) {
        if self.visible.is_empty() {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0);
        let index = (current as isize + delta).clamp(0, self.visible.len() as isize - 1) as usize;
        self.select_list(index);
    }

    pub(super) fn select_list(&mut self, index: usize) {
        if let Some(row) = self.visible.get(index) {
            self.list_state.select(Some(index));
            self.selected_id = Some(row.snippet_id);
            self.preview_target = PreviewTarget::Fragment(0);
            self.preview_scroll = 0;
            self.preview.invalidate();
        }
    }

    /// The switchable positions, fragments first and the README last. With one
    /// to five fragments there is nothing to gain from clamping at the ends, so
    /// both directions wrap: `[` from the first fragment lands on the README.
    fn preview_positions(&self) -> (usize, usize) {
        let snippet = self.selected_snippet();
        let total = snippet.map_or(0, |snippet| snippet.loaded_fragments.len());
        (total, total + usize::from(snippet.is_some_and(has_readme)))
    }

    fn step_preview_target(&mut self, delta: usize) {
        let (total, rows) = self.preview_positions();
        if rows == 0 {
            return;
        }
        let current = match self.preview_target {
            PreviewTarget::Fragment(index) => index.min(rows.saturating_sub(1)),
            PreviewTarget::Readme => total,
        };
        let next = (current + delta) % rows;
        self.preview_target = if next < total {
            PreviewTarget::Fragment(next)
        } else {
            PreviewTarget::Readme
        };
        self.preview_scroll = 0;
        self.preview.invalidate();
    }

    pub(in super::super) fn previous_fragment(&mut self) {
        let (_, rows) = self.preview_positions();
        self.step_preview_target(rows.saturating_sub(1));
    }

    pub(in super::super) fn next_fragment(&mut self) {
        self.step_preview_target(1);
    }

    pub(in super::super) fn toggle_fragments_expanded(&mut self) {
        self.set_fragments_expanded(!self.fragments_expanded);
    }

    pub(in super::super) fn set_fragments_expanded(&mut self, expanded: bool) {
        let can_expand = self
            .selected_snippet()
            .is_some_and(|snippet| !snippet.loaded_fragments.is_empty());
        if !can_expand || self.fragments_expanded == expanded {
            return;
        }
        self.fragments_expanded = expanded;
        self.preview_selection.clear();
    }

    pub(super) fn select_fragment(&mut self, target: PreviewTarget) {
        let (total, rows) = self.preview_positions();
        let target = match target {
            PreviewTarget::Fragment(_) if total == 0 => return,
            PreviewTarget::Fragment(index) => {
                PreviewTarget::Fragment(index.min(total.saturating_sub(1)))
            }
            PreviewTarget::Readme if rows > total => PreviewTarget::Readme,
            PreviewTarget::Readme => return,
        };
        if self.preview_target != target {
            self.preview_target = target;
            self.preview_scroll = 0;
            self.preview.invalidate();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::super::preview::PreviewTarget;
    use super::super::super::types::App;

    /// A three-fragment snippet, optionally carrying a README.
    fn app_with(readme: Option<&str>) -> (tempfile::TempDir, App) {
        let temporary = tempfile::tempdir().unwrap();
        let library =
            crate::filesystem::library::Library::init(&temporary.path().join("T.sniplib"), None)
                .unwrap();
        let created = crate::service::create_snippet(
            &library,
            &crate::service::CreateOptions {
                title: "Snippet".to_owned(),
                language: "rust".to_owned(),
                content: "one\n".to_owned(),
                readme: readme.map(str::to_owned),
                ..crate::service::CreateOptions::default()
            },
        )
        .unwrap();
        for index in 2..=3 {
            crate::service::add_fragment(
                &library,
                &created.id.to_string(),
                &crate::service::FragmentAddOptions {
                    title: format!("Fragment {index}"),
                    language: "rust".to_owned(),
                    content: format!("{index}\n"),
                    ..crate::service::FragmentAddOptions::default()
                },
            )
            .unwrap();
        }
        let app = App::new(library, &crate::config::AppConfig::default()).unwrap();
        (temporary, app)
    }

    #[test]
    fn switching_wraps_through_the_readme() {
        let (_temporary, mut app) = app_with(Some("snippet level prose\n"));
        assert_eq!(app.preview_target, PreviewTarget::Fragment(0));

        app.previous_fragment();
        assert_eq!(app.preview_target, PreviewTarget::Readme);
        app.previous_fragment();
        assert_eq!(app.preview_target, PreviewTarget::Fragment(2));

        app.preview_target = PreviewTarget::Readme;
        app.next_fragment();
        assert_eq!(app.preview_target, PreviewTarget::Fragment(0));
    }

    #[test]
    fn switching_wraps_without_a_readme() {
        let (_temporary, mut app) = app_with(None);
        app.previous_fragment();
        assert_eq!(app.preview_target, PreviewTarget::Fragment(2));
        app.next_fragment();
        assert_eq!(app.preview_target, PreviewTarget::Fragment(0));
    }
}
