use super::{Command, CommandId};
use crate::tui::app::commands::*;

macro_rules! command {
    ($id:ident, $slug:literal, $category:literal, $title:literal, $description:literal, $keywords:expr, $state:ident, $run:ident) => {
        Command {
            id: CommandId::$id,
            slug: $slug,
            category: $category,
            title: $title,
            description: $description,
            keywords: $keywords,
            palette: true,
            state: $state,
            run: $run,
        }
    };
}

macro_rules! key_command {
    ($id:ident, $slug:literal, $category:literal, $title:literal, $description:literal, $run:ident) => {
        Command {
            id: CommandId::$id,
            slug: $slug,
            category: $category,
            title: $title,
            description: $description,
            keywords: &[],
            palette: false,
            state: enabled,
            run: $run,
        }
    };
}

macro_rules! key_command_state {
    ($id:ident, $slug:literal, $category:literal, $title:literal, $description:literal, $state:ident, $run:ident) => {
        Command {
            id: CommandId::$id,
            slug: $slug,
            category: $category,
            title: $title,
            description: $description,
            keywords: &[],
            palette: false,
            state: $state,
            run: $run,
        }
    };
}

pub fn registry() -> &'static [Command] {
    COMMANDS
}

static COMMANDS: &[Command] = &[
    key_command!(
        PaletteOpen,
        "palette.open",
        "UI",
        "Open Palette",
        "Open the command palette",
        palette_open
    ),
    key_command!(
        NavDown,
        "nav.down",
        "Navigation",
        "Move Down",
        "Move to the next item",
        nav_down
    ),
    key_command!(
        NavUp,
        "nav.up",
        "Navigation",
        "Move Up",
        "Move to the previous item",
        nav_up
    ),
    key_command!(
        NavFirst,
        "nav.first",
        "Navigation",
        "Move to First",
        "Move to the first item",
        nav_first
    ),
    key_command!(
        NavLast,
        "nav.last",
        "Navigation",
        "Move to Last",
        "Move to the last item",
        nav_last
    ),
    key_command!(
        NavPageDown,
        "nav.page-down",
        "Navigation",
        "Page Down",
        "Move down by half a page",
        nav_page_down
    ),
    key_command!(
        NavPageUp,
        "nav.page-up",
        "Navigation",
        "Page Up",
        "Move up by half a page",
        nav_page_up
    ),
    key_command!(
        PaneNext,
        "pane.next",
        "Pane",
        "Next Pane",
        "Focus the next pane",
        pane_next
    ),
    key_command!(
        PanePrevious,
        "pane.previous",
        "Pane",
        "Previous Pane",
        "Focus the previous pane",
        pane_previous
    ),
    key_command!(
        PaneBack,
        "pane.back",
        "Pane",
        "Go Back",
        "Go back to the previous pane",
        pane_back
    ),
    key_command!(
        PaneForward,
        "pane.forward",
        "Pane",
        "Go Forward",
        "Drill into the next pane",
        pane_forward
    ),
    key_command!(
        SidebarActivate,
        "sidebar.activate",
        "Sidebar",
        "Activate",
        "Apply the selected library filter",
        sidebar_activate
    ),
    key_command!(
        SidebarToggleFolder,
        "sidebar.toggle-folder",
        "Sidebar",
        "Toggle Folder",
        "Expand or collapse the selected folder",
        sidebar_toggle_folder
    ),
    key_command!(
        SidebarRename,
        "sidebar.rename",
        "Sidebar",
        "Rename",
        "Rename the selected folder or tag",
        sidebar_rename
    ),
    key_command!(
        SidebarDelete,
        "sidebar.delete",
        "Sidebar",
        "Delete",
        "Delete the selected folder or tag",
        sidebar_delete
    ),
    key_command!(
        ListEnterPreview,
        "list.enter-preview",
        "List",
        "Enter Preview",
        "Focus the snippet preview",
        list_enter_preview
    ),
    key_command!(
        PreviewPreviousItem,
        "preview.previous-item",
        "Preview",
        "Previous Item",
        "Select the previous preview item",
        preview_previous_item
    ),
    key_command!(
        PreviewNextItem,
        "preview.next-item",
        "Preview",
        "Next Item",
        "Select the next preview item",
        preview_next_item
    ),
    key_command!(
        PreviewPreviousParagraph,
        "preview.previous-paragraph",
        "Preview",
        "Previous Paragraph",
        "Jump to the previous paragraph",
        preview_previous_paragraph
    ),
    key_command!(
        PreviewNextParagraph,
        "preview.next-paragraph",
        "Preview",
        "Next Paragraph",
        "Jump to the next paragraph",
        preview_next_paragraph
    ),
    key_command!(
        PreviewExpandFragments,
        "preview.expand-fragments",
        "Preview",
        "Expand Fragments",
        "Expand the fragment list",
        preview_expand_fragments
    ),
    key_command!(
        PreviewCollapseFragments,
        "preview.collapse-fragments",
        "Preview",
        "Collapse Fragments",
        "Collapse the fragment list",
        preview_collapse_fragments
    ),
    key_command!(
        GrabDrop,
        "grab.drop",
        "Fragment",
        "Drop Fragment",
        "Drop the grabbed fragment",
        grab_drop
    ),
    key_command!(
        UiDismiss,
        "ui.dismiss",
        "UI",
        "Dismiss",
        "Close the active panel or clear the active filter",
        ui_dismiss
    ),
    command!(
        SnippetNew,
        "snippet.new",
        "Snippet",
        "New Snippet",
        "Create a snippet",
        &["create"],
        enabled,
        snippet_new
    ),
    command!(
        SnippetEditContent,
        "snippet.edit-content",
        "Snippet",
        "Edit Content",
        "Edit the selected fragment content",
        &["edit"],
        has_snippet,
        snippet_edit_content
    ),
    command!(
        SnippetEditNote,
        "snippet.edit-note",
        "Snippet",
        "Edit Note",
        "Edit the selected fragment note",
        &["note"],
        has_snippet,
        snippet_edit_note
    ),
    command!(
        SnippetEditReadme,
        "snippet.edit-readme",
        "Snippet",
        "Edit README",
        "Edit the selected snippet README",
        &["readme"],
        has_snippet,
        snippet_edit_readme
    ),
    command!(
        SnippetOpenVsCode,
        "snippet.open-vscode",
        "Snippet",
        "Open in VS Code",
        "Open the selected fragment in VS Code",
        &["code", "editor"],
        has_snippet,
        snippet_open_vscode
    ),
    command!(
        SnippetRename,
        "snippet.rename",
        "Snippet",
        "Rename",
        "Rename the selected snippet",
        &["title"],
        has_snippet,
        snippet_rename
    ),
    command!(
        SnippetMove,
        "snippet.move",
        "Snippet",
        "Move",
        "Move the selected snippet to another folder",
        &["folder"],
        has_snippet,
        snippet_move
    ),
    command!(
        SnippetEditTags,
        "snippet.edit-tags",
        "Snippet",
        "Edit Tags",
        "Edit the selected snippet tags",
        &["labels"],
        has_snippet,
        snippet_tags
    ),
    command!(
        SnippetEditLanguage,
        "snippet.edit-language",
        "Snippet",
        "Edit Language",
        "Change the selected fragment language",
        &["syntax"],
        has_snippet,
        snippet_language
    ),
    command!(
        SnippetTogglePin,
        "snippet.toggle-pin",
        "Snippet",
        "Toggle Pin",
        "Pin or unpin the selected snippet",
        &["pinned"],
        has_snippet,
        snippet_pin
    ),
    command!(
        SnippetToggleLock,
        "snippet.toggle-lock",
        "Snippet",
        "Toggle Lock",
        "Lock or unlock the selected snippet",
        &["locked"],
        has_snippet,
        snippet_lock
    ),
    command!(
        SnippetMoveToTrash,
        "snippet.move-to-trash",
        "Snippet",
        "Move to Trash",
        "Move the selected snippet to trash",
        &["delete"],
        has_snippet,
        snippet_trash
    ),
    command!(
        FragmentAdd,
        "fragment.add",
        "Fragment",
        "Add Fragment",
        "Add a fragment to the selected snippet",
        &["new", "create"],
        can_add_fragment,
        fragment_add
    ),
    command!(
        FragmentRename,
        "fragment.rename",
        "Fragment",
        "Rename Fragment",
        "Rename the selected fragment",
        &["title"],
        can_edit_fragment,
        fragment_rename
    ),
    command!(
        FragmentReorder,
        "fragment.reorder",
        "Fragment",
        "Move Fragment",
        "Move the selected fragment",
        &["reorder", "order", "position"],
        can_reorder_fragment,
        fragment_reorder
    ),
    command!(
        FragmentRemove,
        "fragment.remove",
        "Fragment",
        "Delete Fragment",
        "Delete the selected fragment",
        &["delete", "remove"],
        can_remove_fragment,
        fragment_remove
    ),
    command!(
        CopyContent,
        "copy.content",
        "Copy",
        "Content",
        "Copy the selected fragment content",
        &["clipboard"],
        has_snippet,
        copy_content
    ),
    command!(
        CopySnippetId,
        "copy.snippet-id",
        "Copy",
        "Snippet ID",
        "Copy the selected snippet ID",
        &["clipboard", "id"],
        has_snippet,
        copy_id
    ),
    command!(
        CopyManagedPath,
        "copy.managed-path",
        "Copy",
        "Managed Path",
        "Copy the selected managed file path",
        &["clipboard", "path"],
        has_snippet,
        copy_path
    ),
    command!(
        FolderNew,
        "folder.new",
        "Folder",
        "New Folder",
        "Create a folder",
        &["create"],
        enabled,
        folder_new
    ),
    command!(
        FolderRename,
        "folder.rename",
        "Folder",
        "Rename",
        "Rename the selected folder",
        &[],
        has_folder,
        folder_rename
    ),
    command!(
        FolderMove,
        "folder.move",
        "Folder",
        "Move",
        "Move the selected folder",
        &[],
        has_folder,
        folder_move
    ),
    command!(
        FolderDelete,
        "folder.delete",
        "Folder",
        "Delete",
        "Delete the selected folder",
        &["remove"],
        has_folder,
        folder_delete
    ),
    command!(
        TagRename,
        "tag.rename",
        "Tag",
        "Rename",
        "Rename the selected tag",
        &[],
        has_tag,
        tag_rename
    ),
    command!(
        TagDelete,
        "tag.delete",
        "Tag",
        "Delete",
        "Delete the selected tag",
        &["remove"],
        has_tag,
        tag_delete
    ),
    command!(
        ViewCycleSort,
        "view.cycle-sort",
        "View",
        "Cycle Sort",
        "Cycle the snippet sort order",
        &[],
        enabled,
        cycle_sort
    ),
    command!(
        ViewSortModified,
        "view.sort-modified",
        "View",
        "Sort by Modified",
        "Sort snippets by modification time",
        &["date"],
        enabled,
        sort_modified
    ),
    command!(
        ViewSortTitle,
        "view.sort-title",
        "View",
        "Sort by Title",
        "Sort snippets by title",
        &["name"],
        enabled,
        sort_title
    ),
    command!(
        ViewSortCreated,
        "view.sort-created",
        "View",
        "Sort by Created",
        "Sort snippets by creation time",
        &["date"],
        enabled,
        sort_created
    ),
    command!(
        ViewToggleLineNumbers,
        "view.toggle-line-numbers",
        "View",
        "Toggle Line Numbers",
        "Show or hide preview line numbers",
        &[],
        enabled,
        toggle_line_numbers
    ),
    command!(
        ViewToggleSimplifiedUi,
        "view.toggle-simplified-ui",
        "View",
        "Toggle Simplified UI",
        "Toggle the simplified terminal UI",
        &["font", "powerline", "nerd font", "square", "bars"],
        enabled,
        toggle_simplified_ui
    ),
    command!(
        ViewToggleFragmentList,
        "view.toggle-fragment-list",
        "View",
        "Toggle Fragment List",
        "Show or hide the fragment list",
        &["fragment", "fragments", "tree", "expand", "collapse"],
        enabled,
        toggle_fragment_list
    ),
    command!(
        ViewToggleDensity,
        "view.toggle-density",
        "View",
        "Toggle Density",
        "Toggle compact snippet list spacing",
        &["list"],
        enabled,
        toggle_density
    ),
    command!(
        ViewToggleSidebar,
        "view.toggle-sidebar",
        "View",
        "Toggle Library Pane",
        "Show or hide the library pane",
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
        "Switch between light and dark appearance",
        &["light", "dark", "appearance", "mode", "force", "theme"],
        enabled,
        cycle_appearance
    ),
    command!(
        ViewClearAppearanceOverride,
        "view.clear-appearance-override",
        "View",
        "Clear Appearance Override",
        "Return appearance selection to automatic",
        &["clear", "override", "auto", "system", "appearance", "reset"],
        enabled,
        clear_appearance_override
    ),
    command!(
        ViewPickTheme,
        "view.pick-theme",
        "View",
        "Change Color Theme",
        "Choose a color theme",
        &["theme", "color", "colour", "scheme", "appearance"],
        enabled,
        view_pick_theme
    ),
    command!(
        ViewToggleHelp,
        "view.toggle-help",
        "View",
        "Toggle Help",
        "Open or close the help panel",
        &[],
        enabled,
        toggle_help
    ),
    key_command!(
        HelpFilter,
        "help.filter",
        "Help",
        "Filter Help",
        "Filter the help cheatsheet",
        help_filter
    ),
    key_command!(
        HelpToggleScope,
        "help.toggle-scope",
        "Help",
        "Toggle Help Scope",
        "Show this context or all modes",
        help_toggle_scope
    ),
    key_command!(
        HelpCycleSort,
        "help.cycle-sort",
        "Help",
        "Cycle Help Sort",
        "Sort help rows by key or action",
        help_cycle_sort
    ),
    command!(
        LibrarySearch,
        "library.search",
        "Library",
        "Search",
        "Search the snippet library",
        &["find"],
        enabled,
        library_search
    ),
    command!(
        LibraryRescan,
        "library.rescan",
        "Library",
        "Rescan",
        "Rescan the snippet library",
        &["refresh"],
        enabled,
        library_rescan
    ),
    command!(
        LibraryToggleTrash,
        "library.toggle-trash",
        "Library",
        "Toggle Trash",
        "Open or close the trash",
        &["deleted"],
        enabled,
        library_toggle_trash
    ),
    command!(
        LibraryClearFilter,
        "library.clear-filter",
        "Library",
        "Clear Filter",
        "Clear the active library filter",
        &["reset"],
        enabled,
        library_clear_filter
    ),
    command!(
        LibraryTogglePublishedFilter,
        "library.toggle-published",
        "Library",
        "Toggle Published Filter",
        "Show or hide published snippets",
        &["gist"],
        enabled,
        library_toggle_published
    ),
    command!(
        GitToggleConsole,
        "git.toggle-console",
        "Git",
        "Toggle Console",
        "Open or close the Git console",
        &["source control"],
        enabled,
        git_toggle_console
    ),
    command!(
        GitBackup,
        "git.backup",
        "Git",
        "Backup",
        "Commit changes and push them to the remote",
        &["commit", "push"],
        git_available,
        git_backup
    ),
    command!(
        GitCommit,
        "git.commit",
        "Git",
        "Commit",
        "Commit library changes",
        &["save"],
        git_available,
        git_commit
    ),
    command!(
        GitCommitWithMessage,
        "git.commit-message",
        "Git",
        "Commit with Message…",
        "Commit library changes with a custom message",
        &["custom"],
        git_available,
        git_message
    ),
    command!(
        GitPush,
        "git.push",
        "Git",
        "Push to Remote",
        "Push local commits to the remote",
        &["upload", "remote"],
        git_available,
        git_push
    ),
    command!(
        GitFetchRemoteStatus,
        "git.fetch",
        "Git",
        "Fetch Remote Status",
        "Fetch the latest remote status",
        &["remote"],
        git_available,
        git_fetch
    ),
    command!(
        GitPull,
        "git.pull",
        "Git",
        "Pull From Remote",
        "Pull changes from the remote",
        &["remote", "sync"],
        git_available,
        git_pull
    ),
    command!(
        GitToggleAutoPull,
        "git.toggle-auto-pull",
        "Git",
        "Toggle Auto Pull",
        "Toggle automatic pull on startup",
        &["automatic"],
        git_available,
        git_auto_pull
    ),
    command!(
        GitRefreshLocalStatus,
        "git.refresh",
        "Git",
        "Refresh Local Status",
        "Refresh local repository status",
        &["status"],
        git_available,
        git_refresh
    ),
    command!(
        GitInitRepository,
        "git.init",
        "Git",
        "Init Repository",
        "Initialize a Git repository",
        &["initialize"],
        can_init_git,
        git_init
    ),
    key_command_state!(
        GitInitOrSetInterval,
        "git.init-or-set-interval",
        "Git",
        "Init or Set Interval",
        "Initialize Git or set the automatic commit interval",
        can_init_or_configure_git,
        git_init_or_set_interval
    ),
    command!(
        GitToggleAutoPush,
        "git.toggle-auto-push",
        "Git",
        "Toggle Auto Push",
        "Toggle automatic push after commits",
        &["automatic"],
        git_available,
        git_auto_push
    ),
    command!(
        GitToggleBackupOnQuit,
        "git.toggle-backup-on-quit",
        "Git",
        "Toggle Backup on Quit",
        "Toggle automatic backup on quit",
        &["automatic"],
        git_available,
        git_backup_on_quit
    ),
    command!(
        GitPauseAutoBackup,
        "git.pause-auto-backup",
        "Git",
        "Pause Auto Backup",
        "Pause automatic backup for this session",
        &["automatic"],
        git_available,
        git_pause
    ),
    command!(
        GitSetAutoCommitInterval,
        "git.set-auto-commit-interval",
        "Git",
        "Set Auto Commit Interval…",
        "Set the automatic commit interval",
        &["automatic"],
        git_available,
        git_interval
    ),
    command!(
        GistTogglePanel,
        "gist.toggle-panel",
        "Gist",
        "Toggle Panel",
        "Open or close the Gist panel",
        &["gist"],
        enabled,
        gist_toggle_panel
    ),
    command!(
        GistPush,
        "gist.push",
        "Gist",
        "Publish or Update",
        "Publish or update the selected snippet as a gist",
        &["share", "gist"],
        has_snippet,
        gist_push
    ),
    command!(
        GistPushPublic,
        "gist.push-public",
        "Gist",
        "Publish as Public Gist",
        "Publish the selected snippet as a public gist",
        &["public"],
        has_snippet,
        gist_push_public
    ),
    command!(
        GistCopyUrl,
        "gist.copy-url",
        "Gist",
        "Copy URL",
        "Copy the selected snippet gist URL",
        &["link", "url"],
        has_gist,
        gist_copy_url
    ),
    command!(
        GistOpenInBrowser,
        "gist.open-browser",
        "Gist",
        "Open in Browser",
        "Open the selected snippet gist in a browser",
        &["gist"],
        has_gist,
        gist_open_browser
    ),
    command!(
        GistAttach,
        "gist.attach",
        "Gist",
        "Attach Existing Gist…",
        "Attach an existing gist to the selected snippet",
        &["adopt", "link"],
        has_snippet,
        gist_attach
    ),
    command!(
        GistDetach,
        "gist.detach",
        "Gist",
        "Detach Gist",
        "Detach the gist from the selected snippet",
        &["gist"],
        has_gist,
        gist_detach
    ),
    command!(
        GistDelete,
        "gist.delete",
        "Gist",
        "Delete Gist…",
        "Delete the selected snippet gist from GitHub",
        &["gist"],
        has_gist,
        gist_delete
    ),
    command!(
        GistVerifyRemote,
        "gist.verify",
        "Gist",
        "Verify Remote",
        "Check that the selected snippet gist still exists",
        &["gist"],
        has_gist,
        gist_verify
    ),
    command!(
        TrashRestoreSelected,
        "trash.restore-selected",
        "Trash",
        "Restore Selected",
        "Restore the selected snippet from trash",
        &["undelete"],
        has_trash_selection,
        trash_restore
    ),
    command!(
        TrashPurgeSelected,
        "trash.purge-selected",
        "Trash",
        "Purge Selected",
        "Permanently delete the selected trashed snippet",
        &["delete", "permanent"],
        has_trash_selection,
        trash_purge
    ),
    command!(
        AppQuit,
        "app.quit",
        "App",
        "Quit",
        "Quit snip",
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

    #[test]
    fn registry_descriptions_are_present_and_well_formed() {
        for command in registry() {
            let description = command.description;
            assert!(!description.trim().is_empty(), "{}", command.slug);
            assert!(description.chars().count() <= 90, "{}", command.slug);
            assert!(!description.ends_with('.'), "{}", command.slug);
            assert!(
                description
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_uppercase()),
                "{}",
                command.slug
            );
        }
    }

    #[test]
    fn command_macros_require_inline_description_literals() {
        let source = include_str!("registry.rs");
        let parameter = ["$description", ":literal"].concat();
        let field = ["description: ", "$description"].concat();
        let distant_function = ["const fn ", "description("].concat();
        assert_eq!(source.matches(&parameter).count(), 3);
        assert_eq!(source.matches(&field).count(), 3);
        assert!(!source.contains(&distant_function));
    }
}
