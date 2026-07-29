use crate::git::{GitAction, Unavailable};
use crate::sort::SortMode;
use crate::tui::app::{App, Effect};
use crate::tui::command::{self, Command, CommandId, CommandState};
use crate::tui::state::{SidebarItem, StatusLevel};

use std::collections::HashSet;

impl App {
    pub fn run_command(&mut self, id: CommandId) -> Vec<Effect> {
        let command = command::get(id);
        match (command.state)(self) {
            CommandState::Enabled => {
                self.palette.record_recent(id);
                (command.run)(self)
            }
            CommandState::Disabled(reason) => {
                self.set_status(reason, StatusLevel::Error);
                Vec::new()
            }
            CommandState::Hidden => Vec::new(),
        }
    }

    pub(super) fn set_sort(&mut self, sort: SortMode) {
        self.sort = sort;
        self.refresh_visible();
    }

    pub(super) fn open_palette(&mut self) {
        self.palette.open();
        self.refresh_palette();
    }

    pub fn refresh_palette(&mut self) {
        let hidden = hidden_command_ids(self, command::registry());
        self.palette.refresh(&hidden);
    }
}

fn hidden_command_ids(app: &App, commands: &[Command]) -> HashSet<CommandId> {
    commands
        .iter()
        .filter(|command| matches!((command.state)(app), CommandState::Hidden))
        .map(|command| command.id)
        .collect()
}

pub(crate) fn enabled(_: &App) -> CommandState {
    CommandState::Enabled
}
pub(crate) fn has_snippet(app: &App) -> CommandState {
    if app.selected_snippet().is_some() {
        CommandState::Enabled
    } else {
        CommandState::Disabled("no snippet selected")
    }
}
pub(crate) fn has_folder(app: &App) -> CommandState {
    if matches!(
        app.sidebar.selected().map(|row| &row.item),
        Some(SidebarItem::Folder(_))
    ) {
        CommandState::Enabled
    } else {
        CommandState::Disabled("no folder selected")
    }
}
pub(crate) fn has_tag(app: &App) -> CommandState {
    if matches!(
        app.sidebar.selected().map(|row| &row.item),
        Some(SidebarItem::Tag(_))
    ) {
        CommandState::Enabled
    } else {
        CommandState::Disabled("no tag selected")
    }
}
pub(crate) fn git_available(app: &App) -> CommandState {
    match app.git.unavailable.as_ref() {
        Some(Unavailable::BinaryMissing) => CommandState::Disabled("git not found in PATH"),
        Some(Unavailable::ProbeFailed { .. }) => {
            CommandState::Disabled("git could not inspect this repository")
        }
        _ if app.git.repo.is_some() => CommandState::Enabled,
        _ => CommandState::Disabled("not a Git repository"),
    }
}
pub(crate) fn can_init_git(app: &App) -> CommandState {
    if matches!(app.git.unavailable, Some(Unavailable::NotARepository)) {
        CommandState::Enabled
    } else {
        CommandState::Disabled("already a Git repository")
    }
}
pub(crate) fn has_trash_selection(app: &App) -> CommandState {
    if app.trash.selected().is_some() {
        CommandState::Enabled
    } else {
        CommandState::Disabled("no trash entry selected")
    }
}

macro_rules! effect {
    ($name:ident, $app:ident => $body:expr) => {
        pub(crate) fn $name($app: &mut App) -> Vec<Effect> {
            $body;
            Vec::new()
        }
    };
}
pub(crate) fn snippet_edit_content(app: &mut App) -> Vec<Effect> {
    app.edit_effect()
}
pub(crate) fn snippet_edit_note(app: &mut App) -> Vec<Effect> {
    app.edit_note_effect()
}
pub(crate) fn snippet_edit_readme(app: &mut App) -> Vec<Effect> {
    app.edit_readme_effect()
}
pub(crate) fn snippet_open_vscode(app: &mut App) -> Vec<Effect> {
    app.open_vscode_effect()
}
pub(crate) fn copy_content(app: &mut App) -> Vec<Effect> {
    app.copy_content_effect()
}
pub(crate) fn copy_id(app: &mut App) -> Vec<Effect> {
    app.copy_id_effect()
}
pub(crate) fn copy_path(app: &mut App) -> Vec<Effect> {
    app.copy_path_effect()
}
pub(crate) fn git_backup(app: &mut App) -> Vec<Effect> {
    app.git_effect(GitAction::Backup)
}
pub(crate) fn git_commit(app: &mut App) -> Vec<Effect> {
    app.git_effect(GitAction::Commit { message: None })
}
pub(crate) fn git_push(app: &mut App) -> Vec<Effect> {
    app.git_effect(GitAction::Push)
}
pub(crate) fn git_init(app: &mut App) -> Vec<Effect> {
    app.git_effect(GitAction::Init)
}
pub(crate) fn app_quit(app: &mut App) -> Vec<Effect> {
    app.request_quit()
}

effect!(snippet_new, app => app.open_new_snippet());
effect!(snippet_rename, app => app.open_rename_snippet());
effect!(snippet_move, app => app.open_move_snippet());
effect!(snippet_tags, app => app.open_edit_tags());
effect!(snippet_language, app => app.open_edit_language());
effect!(snippet_pin, app => app.toggle_pin());
effect!(snippet_lock, app => app.toggle_lock());
effect!(snippet_trash, app => app.open_delete_snippet());
effect!(folder_new, app => app.open_new_folder());
effect!(folder_rename, app => app.open_rename_folder());
effect!(folder_move, app => app.open_move_folder());
effect!(folder_delete, app => app.open_delete_folder());
effect!(tag_rename, app => app.open_rename_tag());
effect!(tag_delete, app => app.open_delete_tag());
effect!(cycle_sort, app => app.set_sort(app.sort.next()));
effect!(sort_modified, app => app.set_sort(SortMode::Modified));
effect!(sort_title, app => app.set_sort(SortMode::Title));
effect!(sort_created, app => app.set_sort(SortMode::Created));
effect!(toggle_line_numbers, app => { app.show_line_numbers = !app.show_line_numbers; app.preview_selection.clear(); app.set_status(if app.show_line_numbers { "line numbers on" } else { "line numbers off" }, StatusLevel::Info); });
effect!(toggle_density, app => app.toggle_density());
effect!(toggle_help, app => { app.show_help = !app.show_help; app.help_scroll = 0; });
effect!(library_search, app => app.search.active = true);
effect!(library_rescan, app => app.rescan_now());
effect!(library_trash, app => app.open_trash());
effect!(library_clear_filter, app => { app.filter = Default::default(); app.search.query.clear(); app.refresh_visible(); });
effect!(git_open, app => { app.show_help = false; app.search.active = false; app.trash.open = false; app.git.open = true; app.refresh_git(); });
effect!(git_message, app => app.open_git_message());
effect!(git_fetch, app => app.spawn_fetch());
effect!(git_refresh, app => app.refresh_git());
effect!(git_auto_push, app => app.toggle_auto_push());
effect!(git_backup_on_quit, app => app.toggle_backup_on_quit());
effect!(git_pause, app => app.toggle_auto_backup());
effect!(git_interval, app => app.open_auto_commit_interval());
effect!(trash_restore, app => app.restore_selected_trash());
effect!(trash_purge, app => app.purge_selected_trash());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::filesystem::Library;

    fn hidden(_: &App) -> CommandState {
        CommandState::Hidden
    }

    fn noop(_: &mut App) -> Vec<Effect> {
        Vec::new()
    }

    #[test]
    fn app_layer_excludes_hidden_commands_before_refreshing_the_palette() {
        let temporary = tempfile::tempdir().unwrap();
        let library = Library::init(&temporary.path().join("Hidden.sniplib"), None).unwrap();
        let mut app = App::new(library, &AppConfig::default()).unwrap();
        let commands = [Command {
            id: CommandId::GitPush,
            slug: "test.hidden",
            category: "Test",
            title: "Hidden",
            keywords: &[],
            key_hint: None,
            state: hidden,
            run: noop,
        }];
        let hidden = hidden_command_ids(&app, &commands);
        app.palette.open();
        app.palette.refresh(&hidden);
        assert!(
            !app.palette
                .matches
                .iter()
                .any(|matched| matched.id == CommandId::GitPush)
        );
    }
}
