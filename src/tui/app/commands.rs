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
pub(crate) fn has_gist(app: &App) -> CommandState {
    if app
        .selected_snippet()
        .is_some_and(|snippet| crate::gist::find(snippet).is_some())
    {
        CommandState::Enabled
    } else {
        CommandState::Disabled("this snippet has no gist")
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

fn fragment_snippet(app: &App) -> Result<&crate::domain::Snippet, CommandState> {
    let snippet = app
        .selected_snippet()
        .ok_or(CommandState::Disabled("no snippet selected"))?;
    if snippet.locked {
        Err(CommandState::Disabled(
            "snippet is locked; use the CLI with --force",
        ))
    } else {
        Ok(snippet)
    }
}

pub(crate) fn can_add_fragment(app: &App) -> CommandState {
    fragment_snippet(app).map_or_else(|state| state, |_| CommandState::Enabled)
}

pub(crate) fn can_edit_fragment(app: &App) -> CommandState {
    let snippet = match fragment_snippet(app) {
        Ok(snippet) => snippet,
        Err(state) => return state,
    };
    if snippet.loaded_fragments.get(app.fragment_index).is_some() {
        CommandState::Enabled
    } else {
        CommandState::Disabled("no fragment selected")
    }
}

pub(crate) fn can_reorder_fragment(app: &App) -> CommandState {
    match can_edit_fragment(app) {
        CommandState::Enabled => {}
        state => return state,
    }
    if app
        .selected_snippet()
        .is_some_and(|snippet| snippet.loaded_fragments.len() >= 2)
    {
        CommandState::Enabled
    } else {
        CommandState::Disabled("only one fragment to move")
    }
}

pub(crate) fn can_remove_fragment(app: &App) -> CommandState {
    match can_edit_fragment(app) {
        CommandState::Enabled => {}
        state => return state,
    }
    if app
        .selected_snippet()
        .is_some_and(|snippet| snippet.loaded_fragments.len() >= 2)
    {
        CommandState::Enabled
    } else {
        CommandState::Disabled("cannot delete the only fragment")
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
effect!(fragment_add, app => app.open_add_fragment());
effect!(fragment_rename, app => app.open_rename_fragment());
effect!(fragment_reorder, app => app.start_fragment_grab());
effect!(fragment_remove, app => app.open_delete_fragment());
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
effect!(toggle_line_numbers, app => app.toggle_line_numbers());
effect!(toggle_fragment_list, app => app.toggle_fragments_expanded());
effect!(toggle_density, app => app.toggle_density());
effect!(view_pick_theme, app => app.open_theme_picker());
effect!(toggle_help, app => { app.show_help = !app.show_help; app.help_scroll = 0; });
effect!(library_search, app => app.search.active = true);
effect!(library_rescan, app => app.rescan_now());
effect!(library_trash, app => app.open_trash());
effect!(library_clear_filter, app => { app.filter = Default::default(); app.search.query.clear(); app.refresh_visible(); });
effect!(git_open, app => { app.show_help = false; app.search.active = false; app.trash.open = false; app.gist.open = false; app.git.open = true; app.refresh_git(); });
effect!(git_message, app => app.open_git_message());
effect!(git_fetch, app => app.spawn_fetch());
effect!(git_pull, app => app.git_effect(crate::git::GitAction::Pull));
effect!(git_refresh, app => app.refresh_git());
effect!(git_auto_push, app => app.toggle_auto_push());
effect!(git_auto_pull, app => app.toggle_auto_pull());
effect!(git_backup_on_quit, app => app.toggle_backup_on_quit());
effect!(git_pause, app => app.toggle_auto_backup());
effect!(git_interval, app => app.open_auto_commit_interval());
effect!(gist_open, app => { app.show_help = false; app.search.active = false; app.trash.open = false; app.git.open = false; app.gist.open = true; });
effect!(gist_push, app => app.push_gist(false));
effect!(gist_push_public, app => app.push_gist(true));
effect!(gist_copy_url, app => app.copy_gist_url());
effect!(gist_open_browser, app => app.open_gist_in_browser());
effect!(gist_attach, app => app.open_gist_attach_modal());
effect!(gist_detach, app => app.open_gist_detach_modal());
effect!(gist_delete, app => app.open_gist_delete_modal());
effect!(gist_verify, app => app.verify_gist());
effect!(library_toggle_published, app => app.toggle_published_filter());
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
