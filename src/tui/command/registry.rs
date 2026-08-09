use super::{Command, CommandId};
use crate::tui::app::commands::*;

macro_rules! command {
    ($id:ident, $slug:literal, $category:literal, $title:literal, $keywords:expr, $state:ident, $run:ident) => {
        Command {
            id: CommandId::$id,
            slug: $slug,
            category: $category,
            title: $title,
            keywords: $keywords,
            palette: true,
            state: $state,
            run: $run,
        }
    };
}

macro_rules! key_command {
    ($id:ident, $slug:literal, $category:literal, $title:literal, $run:ident) => {
        Command {
            id: CommandId::$id,
            slug: $slug,
            category: $category,
            title: $title,
            keywords: &[],
            palette: false,
            state: enabled,
            run: $run,
        }
    };
}

macro_rules! key_command_state {
    ($id:ident, $slug:literal, $category:literal, $title:literal, $state:ident, $run:ident) => {
        Command {
            id: CommandId::$id,
            slug: $slug,
            category: $category,
            title: $title,
            keywords: &[],
            palette: false,
            state: $state,
            run: $run,
        }
    };
}

pub fn registry() -> &'static [Command] {
    &COMMANDS
}

static COMMANDS: [Command; 94] = [
    key_command!(
        PaletteOpen,
        "palette.open",
        "UI",
        "Open Palette",
        palette_open
    ),
    key_command!(NavDown, "nav.down", "Navigation", "Move Down", nav_down),
    key_command!(NavUp, "nav.up", "Navigation", "Move Up", nav_up),
    key_command!(
        NavFirst,
        "nav.first",
        "Navigation",
        "Move to First",
        nav_first
    ),
    key_command!(NavLast, "nav.last", "Navigation", "Move to Last", nav_last),
    key_command!(
        NavPageDown,
        "nav.page-down",
        "Navigation",
        "Page Down",
        nav_page_down
    ),
    key_command!(
        NavPageUp,
        "nav.page-up",
        "Navigation",
        "Page Up",
        nav_page_up
    ),
    key_command!(PaneNext, "pane.next", "Pane", "Next Pane", pane_next),
    key_command!(
        PanePrevious,
        "pane.previous",
        "Pane",
        "Previous Pane",
        pane_previous
    ),
    key_command!(PaneBack, "pane.back", "Pane", "Go Back", pane_back),
    key_command!(
        PaneForward,
        "pane.forward",
        "Pane",
        "Go Forward",
        pane_forward
    ),
    key_command!(
        SidebarActivate,
        "sidebar.activate",
        "Sidebar",
        "Activate",
        sidebar_activate
    ),
    key_command!(
        SidebarToggleFolder,
        "sidebar.toggle-folder",
        "Sidebar",
        "Toggle Folder",
        sidebar_toggle_folder
    ),
    key_command!(
        SidebarRename,
        "sidebar.rename",
        "Sidebar",
        "Rename",
        sidebar_rename
    ),
    key_command!(
        SidebarDelete,
        "sidebar.delete",
        "Sidebar",
        "Delete",
        sidebar_delete
    ),
    key_command!(
        ListEnterPreview,
        "list.enter-preview",
        "List",
        "Enter Preview",
        list_enter_preview
    ),
    key_command!(
        PreviewPreviousItem,
        "preview.previous-item",
        "Preview",
        "Previous Item",
        preview_previous_item
    ),
    key_command!(
        PreviewNextItem,
        "preview.next-item",
        "Preview",
        "Next Item",
        preview_next_item
    ),
    key_command!(
        PreviewPreviousParagraph,
        "preview.previous-paragraph",
        "Preview",
        "Previous Paragraph",
        preview_previous_paragraph
    ),
    key_command!(
        PreviewNextParagraph,
        "preview.next-paragraph",
        "Preview",
        "Next Paragraph",
        preview_next_paragraph
    ),
    key_command!(
        PreviewExpandFragments,
        "preview.expand-fragments",
        "Preview",
        "Expand Fragments",
        preview_expand_fragments
    ),
    key_command!(
        PreviewCollapseFragments,
        "preview.collapse-fragments",
        "Preview",
        "Collapse Fragments",
        preview_collapse_fragments
    ),
    key_command!(
        GrabDrop,
        "grab.drop",
        "Fragment",
        "Drop Fragment",
        grab_drop
    ),
    key_command!(UiDismiss, "ui.dismiss", "UI", "Dismiss", ui_dismiss),
    command!(
        SnippetNew,
        "snippet.new",
        "Snippet",
        "New Snippet",
        &["create"],
        enabled,
        snippet_new
    ),
    command!(
        SnippetEditContent,
        "snippet.edit-content",
        "Snippet",
        "Edit Content",
        &["edit"],
        has_snippet,
        snippet_edit_content
    ),
    command!(
        SnippetEditNote,
        "snippet.edit-note",
        "Snippet",
        "Edit Note",
        &["note"],
        has_snippet,
        snippet_edit_note
    ),
    command!(
        SnippetEditReadme,
        "snippet.edit-readme",
        "Snippet",
        "Edit README",
        &["readme"],
        has_snippet,
        snippet_edit_readme
    ),
    command!(
        SnippetOpenVsCode,
        "snippet.open-vscode",
        "Snippet",
        "Open in VS Code",
        &["code", "editor"],
        has_snippet,
        snippet_open_vscode
    ),
    command!(
        SnippetRename,
        "snippet.rename",
        "Snippet",
        "Rename",
        &["title"],
        has_snippet,
        snippet_rename
    ),
    command!(
        SnippetMove,
        "snippet.move",
        "Snippet",
        "Move",
        &["folder"],
        has_snippet,
        snippet_move
    ),
    command!(
        SnippetEditTags,
        "snippet.edit-tags",
        "Snippet",
        "Edit Tags",
        &["labels"],
        has_snippet,
        snippet_tags
    ),
    command!(
        SnippetEditLanguage,
        "snippet.edit-language",
        "Snippet",
        "Edit Language",
        &["syntax"],
        has_snippet,
        snippet_language
    ),
    command!(
        SnippetTogglePin,
        "snippet.toggle-pin",
        "Snippet",
        "Toggle Pin",
        &["pinned"],
        has_snippet,
        snippet_pin
    ),
    command!(
        SnippetToggleLock,
        "snippet.toggle-lock",
        "Snippet",
        "Toggle Lock",
        &["locked"],
        has_snippet,
        snippet_lock
    ),
    command!(
        SnippetMoveToTrash,
        "snippet.move-to-trash",
        "Snippet",
        "Move to Trash",
        &["delete"],
        has_snippet,
        snippet_trash
    ),
    command!(
        FragmentAdd,
        "fragment.add",
        "Fragment",
        "Add Fragment",
        &["new", "create"],
        can_add_fragment,
        fragment_add
    ),
    command!(
        FragmentRename,
        "fragment.rename",
        "Fragment",
        "Rename Fragment",
        &["title"],
        can_edit_fragment,
        fragment_rename
    ),
    command!(
        FragmentReorder,
        "fragment.reorder",
        "Fragment",
        "Move Fragment",
        &["reorder", "order", "position"],
        can_reorder_fragment,
        fragment_reorder
    ),
    command!(
        FragmentRemove,
        "fragment.remove",
        "Fragment",
        "Delete Fragment",
        &["delete", "remove"],
        can_remove_fragment,
        fragment_remove
    ),
    command!(
        CopyContent,
        "copy.content",
        "Copy",
        "Content",
        &["clipboard"],
        has_snippet,
        copy_content
    ),
    command!(
        CopySnippetId,
        "copy.snippet-id",
        "Copy",
        "Snippet ID",
        &["clipboard", "id"],
        has_snippet,
        copy_id
    ),
    command!(
        CopyManagedPath,
        "copy.managed-path",
        "Copy",
        "Managed Path",
        &["clipboard", "path"],
        has_snippet,
        copy_path
    ),
    command!(
        FolderNew,
        "folder.new",
        "Folder",
        "New Folder",
        &["create"],
        enabled,
        folder_new
    ),
    command!(
        FolderRename,
        "folder.rename",
        "Folder",
        "Rename",
        &[],
        has_folder,
        folder_rename
    ),
    command!(
        FolderMove,
        "folder.move",
        "Folder",
        "Move",
        &[],
        has_folder,
        folder_move
    ),
    command!(
        FolderDelete,
        "folder.delete",
        "Folder",
        "Delete",
        &["remove"],
        has_folder,
        folder_delete
    ),
    command!(
        TagRename,
        "tag.rename",
        "Tag",
        "Rename",
        &[],
        has_tag,
        tag_rename
    ),
    command!(
        TagDelete,
        "tag.delete",
        "Tag",
        "Delete",
        &["remove"],
        has_tag,
        tag_delete
    ),
    command!(
        ViewCycleSort,
        "view.cycle-sort",
        "View",
        "Cycle Sort",
        &[],
        enabled,
        cycle_sort
    ),
    command!(
        ViewSortModified,
        "view.sort-modified",
        "View",
        "Sort by Modified",
        &["date"],
        enabled,
        sort_modified
    ),
    command!(
        ViewSortTitle,
        "view.sort-title",
        "View",
        "Sort by Title",
        &["name"],
        enabled,
        sort_title
    ),
    command!(
        ViewSortCreated,
        "view.sort-created",
        "View",
        "Sort by Created",
        &["date"],
        enabled,
        sort_created
    ),
    command!(
        ViewToggleLineNumbers,
        "view.toggle-line-numbers",
        "View",
        "Toggle Line Numbers",
        &[],
        enabled,
        toggle_line_numbers
    ),
    command!(
        ViewToggleSimplifiedUi,
        "view.toggle-simplified-ui",
        "View",
        "Toggle Simplified UI",
        &["font", "powerline", "nerd font", "square", "bars"],
        enabled,
        toggle_simplified_ui
    ),
    command!(
        ViewToggleFragmentList,
        "view.toggle-fragment-list",
        "View",
        "Toggle Fragment List",
        &["fragment", "fragments", "tree", "expand", "collapse"],
        enabled,
        toggle_fragment_list
    ),
    command!(
        ViewToggleDensity,
        "view.toggle-density",
        "View",
        "Toggle Density",
        &["list"],
        enabled,
        toggle_density
    ),
    command!(
        ViewToggleSidebar,
        "view.toggle-sidebar",
        "View",
        "Toggle Library Pane",
        &[
            "sidebar", "library", "pane", "hide", "show", "collapse", "width", "narrow"
        ],
        enabled,
        toggle_sidebar
    ),
    command!(
        ViewCycleAppearance,
        "view.cycle-appearance",
        "View",
        "Toggle Light / Dark",
        &["light", "dark", "appearance", "mode", "force", "theme"],
        enabled,
        cycle_appearance
    ),
    command!(
        ViewClearAppearanceOverride,
        "view.clear-appearance-override",
        "View",
        "Clear Appearance Override",
        &["clear", "override", "auto", "system", "appearance", "reset"],
        enabled,
        clear_appearance_override
    ),
    command!(
        ViewPickTheme,
        "view.pick-theme",
        "View",
        "Change Color Theme",
        &["theme", "color", "colour", "scheme", "appearance"],
        enabled,
        view_pick_theme
    ),
    command!(
        ViewToggleHelp,
        "view.toggle-help",
        "View",
        "Toggle Help",
        &[],
        enabled,
        toggle_help
    ),
    command!(
        LibrarySearch,
        "library.search",
        "Library",
        "Search",
        &["find"],
        enabled,
        library_search
    ),
    command!(
        LibraryRescan,
        "library.rescan",
        "Library",
        "Rescan",
        &["refresh"],
        enabled,
        library_rescan
    ),
    command!(
        LibraryToggleTrash,
        "library.toggle-trash",
        "Library",
        "Toggle Trash",
        &["deleted"],
        enabled,
        library_toggle_trash
    ),
    command!(
        LibraryClearFilter,
        "library.clear-filter",
        "Library",
        "Clear Filter",
        &["reset"],
        enabled,
        library_clear_filter
    ),
    command!(
        LibraryTogglePublishedFilter,
        "library.toggle-published",
        "Library",
        "Toggle Published Filter",
        &["gist"],
        enabled,
        library_toggle_published
    ),
    command!(
        GitToggleConsole,
        "git.toggle-console",
        "Git",
        "Toggle Console",
        &["source control"],
        enabled,
        git_toggle_console
    ),
    command!(
        GitBackup,
        "git.backup",
        "Git",
        "Backup",
        &["commit", "push"],
        git_available,
        git_backup
    ),
    command!(
        GitCommit,
        "git.commit",
        "Git",
        "Commit",
        &["save"],
        git_available,
        git_commit
    ),
    command!(
        GitCommitWithMessage,
        "git.commit-message",
        "Git",
        "Commit with Message…",
        &["custom"],
        git_available,
        git_message
    ),
    command!(
        GitPush,
        "git.push",
        "Git",
        "Push to Remote",
        &["upload", "remote"],
        git_available,
        git_push
    ),
    command!(
        GitFetchRemoteStatus,
        "git.fetch",
        "Git",
        "Fetch Remote Status",
        &["remote"],
        git_available,
        git_fetch
    ),
    command!(
        GitPull,
        "git.pull",
        "Git",
        "Pull From Remote",
        &["remote", "sync"],
        git_available,
        git_pull
    ),
    command!(
        GitToggleAutoPull,
        "git.toggle-auto-pull",
        "Git",
        "Toggle Auto Pull",
        &["automatic"],
        git_available,
        git_auto_pull
    ),
    command!(
        GitRefreshLocalStatus,
        "git.refresh",
        "Git",
        "Refresh Local Status",
        &["status"],
        git_available,
        git_refresh
    ),
    command!(
        GitInitRepository,
        "git.init",
        "Git",
        "Init Repository",
        &["initialize"],
        can_init_git,
        git_init
    ),
    key_command_state!(
        GitInitOrSetInterval,
        "git.init-or-set-interval",
        "Git",
        "Init or Set Interval",
        can_init_or_configure_git,
        git_init_or_set_interval
    ),
    command!(
        GitToggleAutoPush,
        "git.toggle-auto-push",
        "Git",
        "Toggle Auto Push",
        &["automatic"],
        git_available,
        git_auto_push
    ),
    command!(
        GitToggleBackupOnQuit,
        "git.toggle-backup-on-quit",
        "Git",
        "Toggle Backup on Quit",
        &["automatic"],
        git_available,
        git_backup_on_quit
    ),
    command!(
        GitPauseAutoBackup,
        "git.pause-auto-backup",
        "Git",
        "Pause Auto Backup",
        &["automatic"],
        git_available,
        git_pause
    ),
    command!(
        GitSetAutoCommitInterval,
        "git.set-auto-commit-interval",
        "Git",
        "Set Auto Commit Interval…",
        &["automatic"],
        git_available,
        git_interval
    ),
    command!(
        GistTogglePanel,
        "gist.toggle-panel",
        "Gist",
        "Toggle Panel",
        &["gist"],
        enabled,
        gist_toggle_panel
    ),
    command!(
        GistPush,
        "gist.push",
        "Gist",
        "Publish or Update",
        &["share", "gist"],
        has_snippet,
        gist_push
    ),
    command!(
        GistPushPublic,
        "gist.push-public",
        "Gist",
        "Publish as Public Gist",
        &["public"],
        has_snippet,
        gist_push_public
    ),
    command!(
        GistCopyUrl,
        "gist.copy-url",
        "Gist",
        "Copy URL",
        &["link", "url"],
        has_gist,
        gist_copy_url
    ),
    command!(
        GistOpenInBrowser,
        "gist.open-browser",
        "Gist",
        "Open in Browser",
        &["gist"],
        has_gist,
        gist_open_browser
    ),
    command!(
        GistAttach,
        "gist.attach",
        "Gist",
        "Attach Existing Gist…",
        &["adopt", "link"],
        has_snippet,
        gist_attach
    ),
    command!(
        GistDetach,
        "gist.detach",
        "Gist",
        "Detach Gist",
        &["gist"],
        has_gist,
        gist_detach
    ),
    command!(
        GistDelete,
        "gist.delete",
        "Gist",
        "Delete Gist…",
        &["gist"],
        has_gist,
        gist_delete
    ),
    command!(
        GistVerifyRemote,
        "gist.verify",
        "Gist",
        "Verify Remote",
        &["gist"],
        has_gist,
        gist_verify
    ),
    command!(
        TrashRestoreSelected,
        "trash.restore-selected",
        "Trash",
        "Restore Selected",
        &["undelete"],
        has_trash_selection,
        trash_restore
    ),
    command!(
        TrashPurgeSelected,
        "trash.purge-selected",
        "Trash",
        "Purge Selected",
        &["delete", "permanent"],
        has_trash_selection,
        trash_purge
    ),
    command!(
        AppQuit,
        "app.quit",
        "App",
        "Quit",
        &["exit"],
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
