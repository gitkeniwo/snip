use super::{Command, CommandId};
use crate::tui::app::commands::*;

macro_rules! command {
    ($id:ident, $slug:literal, $category:literal, $title:literal, $keywords:expr, $key:expr, $state:ident, $run:ident) => {
        Command {
            id: CommandId::$id,
            slug: $slug,
            category: $category,
            title: $title,
            keywords: $keywords,
            key_hint: $key,
            state: $state,
            run: $run,
        }
    };
}

pub fn registry() -> &'static [Command] {
    &COMMANDS
}

static COMMANDS: [Command; 48] = [
    command!(
        SnippetNew,
        "snippet.new",
        "Snippet",
        "New Snippet",
        &["create"],
        Some("n"),
        enabled,
        snippet_new
    ),
    command!(
        SnippetEditContent,
        "snippet.edit-content",
        "Snippet",
        "Edit Content",
        &["edit"],
        Some("e"),
        has_snippet,
        snippet_edit_content
    ),
    command!(
        SnippetEditNote,
        "snippet.edit-note",
        "Snippet",
        "Edit Note",
        &["note"],
        Some("E"),
        has_snippet,
        snippet_edit_note
    ),
    command!(
        SnippetEditReadme,
        "snippet.edit-readme",
        "Snippet",
        "Edit README",
        &["readme"],
        Some("R"),
        has_snippet,
        snippet_edit_readme
    ),
    command!(
        SnippetOpenVsCode,
        "snippet.open-vscode",
        "Snippet",
        "Open in VS Code",
        &["code", "editor"],
        Some("v"),
        has_snippet,
        snippet_open_vscode
    ),
    command!(
        SnippetRename,
        "snippet.rename",
        "Snippet",
        "Rename",
        &["title"],
        Some("r"),
        has_snippet,
        snippet_rename
    ),
    command!(
        SnippetMove,
        "snippet.move",
        "Snippet",
        "Move",
        &["folder"],
        Some("m"),
        has_snippet,
        snippet_move
    ),
    command!(
        SnippetEditTags,
        "snippet.edit-tags",
        "Snippet",
        "Edit Tags",
        &["labels"],
        Some("t"),
        has_snippet,
        snippet_tags
    ),
    command!(
        SnippetEditLanguage,
        "snippet.edit-language",
        "Snippet",
        "Edit Language",
        &["syntax"],
        Some("f"),
        has_snippet,
        snippet_language
    ),
    command!(
        SnippetTogglePin,
        "snippet.toggle-pin",
        "Snippet",
        "Toggle Pin",
        &["pinned"],
        Some("P"),
        has_snippet,
        snippet_pin
    ),
    command!(
        SnippetToggleLock,
        "snippet.toggle-lock",
        "Snippet",
        "Toggle Lock",
        &["locked"],
        Some("L"),
        has_snippet,
        snippet_lock
    ),
    command!(
        SnippetMoveToTrash,
        "snippet.move-to-trash",
        "Snippet",
        "Move to Trash",
        &["delete"],
        Some("d"),
        has_snippet,
        snippet_trash
    ),
    command!(
        CopyContent,
        "copy.content",
        "Copy",
        "Content",
        &["clipboard"],
        Some("y"),
        has_snippet,
        copy_content
    ),
    command!(
        CopySnippetId,
        "copy.snippet-id",
        "Copy",
        "Snippet ID",
        &["clipboard", "id"],
        Some("Y"),
        has_snippet,
        copy_id
    ),
    command!(
        CopyManagedPath,
        "copy.managed-path",
        "Copy",
        "Managed Path",
        &["clipboard", "path"],
        Some("p"),
        has_snippet,
        copy_path
    ),
    command!(
        FolderNew,
        "folder.new",
        "Folder",
        "New Folder",
        &["create"],
        Some("n"),
        enabled,
        folder_new
    ),
    command!(
        FolderRename,
        "folder.rename",
        "Folder",
        "Rename",
        &[],
        Some("r"),
        has_folder,
        folder_rename
    ),
    command!(
        FolderMove,
        "folder.move",
        "Folder",
        "Move",
        &[],
        Some("m"),
        has_folder,
        folder_move
    ),
    command!(
        FolderDelete,
        "folder.delete",
        "Folder",
        "Delete",
        &["remove"],
        Some("d"),
        has_folder,
        folder_delete
    ),
    command!(
        TagRename,
        "tag.rename",
        "Tag",
        "Rename",
        &[],
        Some("r"),
        has_tag,
        tag_rename
    ),
    command!(
        TagDelete,
        "tag.delete",
        "Tag",
        "Delete",
        &["remove"],
        Some("d"),
        has_tag,
        tag_delete
    ),
    command!(
        ViewCycleSort,
        "view.cycle-sort",
        "View",
        "Cycle Sort",
        &[],
        Some("s"),
        enabled,
        cycle_sort
    ),
    command!(
        ViewSortModified,
        "view.sort-modified",
        "View",
        "Sort by Modified",
        &["date"],
        None,
        enabled,
        sort_modified
    ),
    command!(
        ViewSortTitle,
        "view.sort-title",
        "View",
        "Sort by Title",
        &["name"],
        None,
        enabled,
        sort_title
    ),
    command!(
        ViewSortCreated,
        "view.sort-created",
        "View",
        "Sort by Created",
        &["date"],
        None,
        enabled,
        sort_created
    ),
    command!(
        ViewToggleLineNumbers,
        "view.toggle-line-numbers",
        "View",
        "Toggle Line Numbers",
        &[],
        Some("N"),
        enabled,
        toggle_line_numbers
    ),
    command!(
        ViewToggleDensity,
        "view.toggle-density",
        "View",
        "Toggle Density",
        &["list"],
        Some("z"),
        enabled,
        toggle_density
    ),
    command!(
        ViewPickTheme,
        "view.pick-theme",
        "View",
        "Change Color Theme",
        &["theme", "color", "colour", "scheme", "appearance"],
        None,
        enabled,
        view_pick_theme
    ),
    command!(
        ViewToggleHelp,
        "view.toggle-help",
        "View",
        "Toggle Help",
        &[],
        Some("?"),
        enabled,
        toggle_help
    ),
    command!(
        LibrarySearch,
        "library.search",
        "Library",
        "Search",
        &["find"],
        Some("/"),
        enabled,
        library_search
    ),
    command!(
        LibraryRescan,
        "library.rescan",
        "Library",
        "Rescan",
        &["refresh"],
        Some("F5 / Ctrl-r"),
        enabled,
        library_rescan
    ),
    command!(
        LibraryOpenTrash,
        "library.open-trash",
        "Library",
        "Open Trash",
        &["deleted"],
        Some("T"),
        enabled,
        library_trash
    ),
    command!(
        LibraryClearFilter,
        "library.clear-filter",
        "Library",
        "Clear Filter",
        &["reset"],
        None,
        enabled,
        library_clear_filter
    ),
    command!(
        GitOpenConsole,
        "git.open-console",
        "Git",
        "Open Console",
        &["source control"],
        Some("Ctrl-g"),
        enabled,
        git_open
    ),
    command!(
        GitBackup,
        "git.backup",
        "Git",
        "Backup",
        &["commit", "push"],
        Some("Ctrl-g b"),
        git_available,
        git_backup
    ),
    command!(
        GitCommit,
        "git.commit",
        "Git",
        "Commit",
        &["save"],
        Some("Ctrl-g c"),
        git_available,
        git_commit
    ),
    command!(
        GitCommitWithMessage,
        "git.commit-message",
        "Git",
        "Commit with Message…",
        &["custom"],
        Some("Ctrl-g C"),
        git_available,
        git_message
    ),
    command!(
        GitPush,
        "git.push",
        "Git",
        "Push to Remote",
        &["upload", "remote"],
        Some("Ctrl-g p"),
        git_available,
        git_push
    ),
    command!(
        GitFetchRemoteStatus,
        "git.fetch",
        "Git",
        "Fetch Remote Status",
        &["remote"],
        Some("Ctrl-g f"),
        git_available,
        git_fetch
    ),
    command!(
        GitRefreshLocalStatus,
        "git.refresh",
        "Git",
        "Refresh Local Status",
        &["status"],
        Some("Ctrl-g r"),
        git_available,
        git_refresh
    ),
    command!(
        GitInitRepository,
        "git.init",
        "Git",
        "Init Repository",
        &["initialize"],
        Some("Ctrl-g i"),
        can_init_git,
        git_init
    ),
    command!(
        GitToggleAutoPush,
        "git.toggle-auto-push",
        "Git",
        "Toggle Auto Push",
        &["automatic"],
        Some("Ctrl-g u"),
        git_available,
        git_auto_push
    ),
    command!(
        GitToggleBackupOnQuit,
        "git.toggle-backup-on-quit",
        "Git",
        "Toggle Backup on Quit",
        &["automatic"],
        Some("Ctrl-g o"),
        git_available,
        git_backup_on_quit
    ),
    command!(
        GitPauseAutoBackup,
        "git.pause-auto-backup",
        "Git",
        "Pause Auto Backup",
        &["automatic"],
        Some("Ctrl-g a"),
        git_available,
        git_pause
    ),
    command!(
        GitSetAutoCommitInterval,
        "git.set-auto-commit-interval",
        "Git",
        "Set Auto Commit Interval…",
        &["automatic"],
        Some("Ctrl-g i"),
        git_available,
        git_interval
    ),
    command!(
        TrashRestoreSelected,
        "trash.restore-selected",
        "Trash",
        "Restore Selected",
        &["undelete"],
        Some("u"),
        has_trash_selection,
        trash_restore
    ),
    command!(
        TrashPurgeSelected,
        "trash.purge-selected",
        "Trash",
        "Purge Selected",
        &["delete", "permanent"],
        Some("x"),
        has_trash_selection,
        trash_purge
    ),
    command!(
        AppQuit,
        "app.quit",
        "App",
        "Quit",
        &["exit"],
        Some("q"),
        enabled,
        app_quit
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    #[test]
    fn registry_is_complete_and_slugs_are_unique() {
        let registered_ids = registry()
            .iter()
            .map(|command| command.id)
            .collect::<HashSet<_>>();
        let declared_ids = CommandId::ALL.iter().copied().collect::<HashSet<_>>();
        let slugs = registry()
            .iter()
            .map(|command| command.slug)
            .collect::<HashSet<_>>();
        assert_eq!(declared_ids.len(), CommandId::ALL.len());
        assert_eq!(registered_ids, declared_ids);
        assert_eq!(slugs.len(), registry().len());
    }
}
