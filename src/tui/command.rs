mod registry;

use std::borrow::Cow;
use std::path::Path;

use crate::tui::app::{App, Effect};

pub use registry::registry;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CommandId {
    PaletteOpen,
    NavDown,
    NavUp,
    NavFirst,
    NavLast,
    NavPageDown,
    NavPageUp,
    PaneNext,
    PanePrevious,
    PaneBack,
    PaneForward,
    SidebarActivate,
    SidebarToggleFolder,
    SidebarRename,
    SidebarDelete,
    ListEnterPreview,
    PreviewPreviousItem,
    PreviewNextItem,
    PreviewPreviousParagraph,
    PreviewNextParagraph,
    PreviewExpandFragments,
    PreviewCollapseFragments,
    GrabDrop,
    UiDismiss,
    SnippetNew,
    SnippetEditContent,
    SnippetEditNote,
    SnippetEditReadme,
    SnippetOpenGui,
    SnippetRename,
    SnippetMove,
    SnippetEditTags,
    SnippetEditLanguage,
    SnippetTogglePin,
    SnippetToggleLock,
    SnippetMoveToTrash,
    FragmentAdd,
    FragmentRename,
    FragmentReorder,
    FragmentRemove,
    CopyContent,
    CopySnippetId,
    CopyManagedPath,
    FolderNew,
    FolderRename,
    FolderMove,
    FolderDelete,
    TagRename,
    TagDelete,
    ViewCycleSort,
    ViewSortModified,
    ViewSortTitle,
    ViewSortCreated,
    ViewToggleLineNumbers,
    ViewToggleSimplifiedUi,
    ViewToggleFragmentList,
    ViewToggleDensity,
    ViewToggleSidebar,
    ViewCycleAppearance,
    ViewClearAppearanceOverride,
    ViewPickTheme,
    ViewToggleHelp,
    HelpFilter,
    HelpToggleScope,
    HelpCycleSort,
    LibrarySearch,
    LibraryRescan,
    LibraryToggleTrash,
    LibraryClearFilter,
    LibraryTogglePublishedFilter,
    GitToggleConsole,
    GitBackup,
    GitCommit,
    GitCommitWithMessage,
    GitPush,
    GitFetchRemoteStatus,
    GitPull,
    GitToggleAutoPull,
    GitRefreshLocalStatus,
    GitInitRepository,
    GitInitOrSetInterval,
    GitToggleAutoPush,
    GitToggleBackupOnQuit,
    GitPauseAutoBackup,
    GitSetAutoCommitInterval,
    GistTogglePanel,
    GistPush,
    GistPushPublic,
    GistCopyUrl,
    GistOpenInBrowser,
    GistAttach,
    GistDetach,
    GistDelete,
    GistVerifyRemote,
    TrashRestoreSelected,
    TrashPurgeSelected,
    AppQuit,
}

impl CommandId {
    pub const ALL: &'static [Self] = &[
        Self::PaletteOpen,
        Self::NavDown,
        Self::NavUp,
        Self::NavFirst,
        Self::NavLast,
        Self::NavPageDown,
        Self::NavPageUp,
        Self::PaneNext,
        Self::PanePrevious,
        Self::PaneBack,
        Self::PaneForward,
        Self::SidebarActivate,
        Self::SidebarToggleFolder,
        Self::SidebarRename,
        Self::SidebarDelete,
        Self::ListEnterPreview,
        Self::PreviewPreviousItem,
        Self::PreviewNextItem,
        Self::PreviewPreviousParagraph,
        Self::PreviewNextParagraph,
        Self::PreviewExpandFragments,
        Self::PreviewCollapseFragments,
        Self::GrabDrop,
        Self::UiDismiss,
        Self::SnippetNew,
        Self::SnippetEditContent,
        Self::SnippetEditNote,
        Self::SnippetEditReadme,
        Self::SnippetOpenGui,
        Self::SnippetRename,
        Self::SnippetMove,
        Self::SnippetEditTags,
        Self::SnippetEditLanguage,
        Self::SnippetTogglePin,
        Self::SnippetToggleLock,
        Self::SnippetMoveToTrash,
        Self::FragmentAdd,
        Self::FragmentRename,
        Self::FragmentReorder,
        Self::FragmentRemove,
        Self::CopyContent,
        Self::CopySnippetId,
        Self::CopyManagedPath,
        Self::FolderNew,
        Self::FolderRename,
        Self::FolderMove,
        Self::FolderDelete,
        Self::TagRename,
        Self::TagDelete,
        Self::ViewCycleSort,
        Self::ViewSortModified,
        Self::ViewSortTitle,
        Self::ViewSortCreated,
        Self::ViewToggleLineNumbers,
        Self::ViewToggleSimplifiedUi,
        Self::ViewToggleFragmentList,
        Self::ViewToggleDensity,
        Self::ViewToggleSidebar,
        Self::ViewCycleAppearance,
        Self::ViewClearAppearanceOverride,
        Self::ViewPickTheme,
        Self::ViewToggleHelp,
        Self::HelpFilter,
        Self::HelpToggleScope,
        Self::HelpCycleSort,
        Self::LibrarySearch,
        Self::LibraryRescan,
        Self::LibraryToggleTrash,
        Self::LibraryClearFilter,
        Self::LibraryTogglePublishedFilter,
        Self::GitToggleConsole,
        Self::GitBackup,
        Self::GitCommit,
        Self::GitCommitWithMessage,
        Self::GitPush,
        Self::GitFetchRemoteStatus,
        Self::GitPull,
        Self::GitToggleAutoPull,
        Self::GitRefreshLocalStatus,
        Self::GitInitRepository,
        Self::GitInitOrSetInterval,
        Self::GitToggleAutoPush,
        Self::GitToggleBackupOnQuit,
        Self::GitPauseAutoBackup,
        Self::GitSetAutoCommitInterval,
        Self::GistTogglePanel,
        Self::GistPush,
        Self::GistPushPublic,
        Self::GistCopyUrl,
        Self::GistOpenInBrowser,
        Self::GistAttach,
        Self::GistDetach,
        Self::GistDelete,
        Self::GistVerifyRemote,
        Self::TrashRestoreSelected,
        Self::TrashPurgeSelected,
        Self::AppQuit,
    ];
}

pub struct Command {
    pub id: CommandId,
    pub slug: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub palette: bool,
    pub state: fn(&App) -> CommandState,
    pub run: fn(&mut App) -> Vec<Effect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandState {
    Enabled,
    Disabled(&'static str),
    Hidden,
}

pub fn get(id: CommandId) -> &'static Command {
    registry()
        .iter()
        .find(|command| command.id == id)
        .expect("every CommandId is registered")
}

pub fn by_slug(slug: &str) -> Option<CommandId> {
    registry()
        .iter()
        .find(|command| command.slug == slug)
        .map(|command| command.id)
}

pub fn resolve_slug(slug: &str) -> Option<(CommandId, Option<&'static str>)> {
    by_slug(slug).map(|id| (id, None)).or_else(|| {
        registry::DEPRECATED_SLUGS
            .iter()
            .find(|(deprecated, _)| *deprecated == slug)
            .map(|(_, id)| (*id, Some(get(*id).slug)))
    })
}

pub fn display_title(command: &Command, gui_editor: Option<&str>) -> Cow<'static, str> {
    if command.id == CommandId::SnippetOpenGui
        && let Some(name) = gui_editor_name(gui_editor)
    {
        return Cow::Owned(format!("Open in {name}"));
    }
    Cow::Borrowed(command.title)
}

pub fn gui_editor_name(gui_editor: Option<&str>) -> Option<String> {
    let parts = shlex::split(gui_editor?)?;
    let executable = parts.first()?;
    Path::new(executable)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{CommandId, display_title, get};

    #[test]
    fn gui_editor_title_uses_the_configured_executable_name() {
        let command = get(CommandId::SnippetOpenGui);
        assert_eq!(display_title(command, Some("zed")), "Open in zed");
        assert_eq!(
            display_title(command, Some("/usr/local/bin/code -w")),
            "Open in code"
        );
        assert_eq!(display_title(command, None), "Open in GUI Editor");
        assert_eq!(display_title(command, Some("'")), "Open in GUI Editor");
        assert_eq!(
            display_title(get(CommandId::SnippetRename), Some("zed")),
            "Rename"
        );
    }
}
