mod registry;

use crate::tui::app::{App, Effect};

pub use registry::registry;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CommandId {
    SnippetNew,
    SnippetEditContent,
    SnippetEditNote,
    SnippetEditReadme,
    SnippetOpenVsCode,
    SnippetRename,
    SnippetMove,
    SnippetEditTags,
    SnippetEditLanguage,
    SnippetTogglePin,
    SnippetToggleLock,
    SnippetMoveToTrash,
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
    ViewToggleFragmentList,
    ViewToggleDensity,
    ViewPickTheme,
    ViewToggleHelp,
    LibrarySearch,
    LibraryRescan,
    LibraryOpenTrash,
    LibraryClearFilter,
    LibraryTogglePublishedFilter,
    GitOpenConsole,
    GitBackup,
    GitCommit,
    GitCommitWithMessage,
    GitPush,
    GitFetchRemoteStatus,
    GitRefreshLocalStatus,
    GitInitRepository,
    GitToggleAutoPush,
    GitToggleBackupOnQuit,
    GitPauseAutoBackup,
    GitSetAutoCommitInterval,
    GistOpenPanel,
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
        Self::SnippetNew,
        Self::SnippetEditContent,
        Self::SnippetEditNote,
        Self::SnippetEditReadme,
        Self::SnippetOpenVsCode,
        Self::SnippetRename,
        Self::SnippetMove,
        Self::SnippetEditTags,
        Self::SnippetEditLanguage,
        Self::SnippetTogglePin,
        Self::SnippetToggleLock,
        Self::SnippetMoveToTrash,
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
        Self::ViewToggleFragmentList,
        Self::ViewToggleDensity,
        Self::ViewPickTheme,
        Self::ViewToggleHelp,
        Self::LibrarySearch,
        Self::LibraryRescan,
        Self::LibraryOpenTrash,
        Self::LibraryClearFilter,
        Self::LibraryTogglePublishedFilter,
        Self::GitOpenConsole,
        Self::GitBackup,
        Self::GitCommit,
        Self::GitCommitWithMessage,
        Self::GitPush,
        Self::GitFetchRemoteStatus,
        Self::GitRefreshLocalStatus,
        Self::GitInitRepository,
        Self::GitToggleAutoPush,
        Self::GitToggleBackupOnQuit,
        Self::GitPauseAutoBackup,
        Self::GitSetAutoCommitInterval,
        Self::GistOpenPanel,
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
    pub keywords: &'static [&'static str],
    pub key_hint: Option<&'static str>,
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
