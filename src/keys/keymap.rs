use std::collections::HashMap;

use crate::tui::app::App;
use crate::tui::command::CommandId;
use crate::tui::state::Pane;

use super::Chord;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Mode {
    Global,
    Sidebar,
    List,
    Preview,
    Fragment,
    FragmentGrab,
    Trash,
    Help,
    Git,
    Gist,
    Search,
}

impl Mode {
    pub const ALL: [Self; 11] = [
        Self::Global,
        Self::Sidebar,
        Self::List,
        Self::Preview,
        Self::Fragment,
        Self::FragmentGrab,
        Self::Trash,
        Self::Help,
        Self::Git,
        Self::Gist,
        Self::Search,
    ];

    pub const CONFIGURABLE: [Self; 10] = [
        Self::Global,
        Self::Sidebar,
        Self::List,
        Self::Preview,
        Self::Fragment,
        Self::FragmentGrab,
        Self::Trash,
        Self::Help,
        Self::Git,
        Self::Gist,
    ];

    pub fn stack(app: &App) -> Vec<Self> {
        if app.git.open {
            vec![Self::Git]
        } else if app.gist.open {
            vec![Self::Gist]
        } else if app.search.active {
            // Search owns the keyboard, but the two panels stay reachable from
            // inside a query, as they were before the table took over.
            vec![Self::Search]
        } else if app.show_help {
            // Help deliberately wins the one real overlap: help over trash.
            vec![Self::Help]
        } else if app.trash.open && app.focus == Pane::List {
            vec![Self::Trash]
        } else if app.fragment_grab.is_some() {
            vec![Self::FragmentGrab]
        } else {
            match app.focus {
                Pane::Sidebar => vec![Self::Sidebar, Self::Global],
                Pane::List => vec![Self::List, Self::Global],
                Pane::Preview if app.fragment_context() => {
                    vec![Self::Fragment, Self::Preview, Self::Global]
                }
                Pane::Preview => vec![Self::Preview, Self::Global],
            }
        }
    }

    pub fn is_exclusive(self) -> bool {
        matches!(
            self,
            Self::FragmentGrab | Self::Trash | Self::Help | Self::Git | Self::Gist | Self::Search
        )
    }

    pub fn inherits(self) -> &'static [CommandId] {
        use CommandId::*;
        match self {
            Self::Git | Self::Gist => &[GitToggleConsole, GistTogglePanel, PaletteOpen],
            Self::Trash => &[
                LibraryToggleTrash,
                GitToggleConsole,
                GistTogglePanel,
                PaletteOpen,
            ],
            Self::Help => &[
                ViewToggleHelp,
                GitToggleConsole,
                GistTogglePanel,
                PaletteOpen,
            ],
            Self::FragmentGrab | Self::Search => &[GitToggleConsole, GistTogglePanel],
            Self::Global | Self::Sidebar | Self::List | Self::Preview | Self::Fragment => &[],
        }
    }
}

pub struct Keymap {
    pub(super) modes: HashMap<Mode, HashMap<Chord, CommandId>>,
}

impl Keymap {
    pub(super) fn empty() -> Self {
        Self {
            modes: HashMap::new(),
        }
    }

    pub fn defaults() -> Self {
        use CommandId::*;
        use Mode::*;

        let mut keymap = Self {
            modes: HashMap::new(),
        };

        keymap.bind(Global, PaletteOpen, &[":", "ctrl-p"]);
        keymap.bind(Global, AppQuit, &["q"]);
        keymap.bind(Global, PaneNext, &["tab"]);
        keymap.bind(Global, PanePrevious, &["backtab"]);
        keymap.bind(Global, PaneBack, &["h", "left"]);
        keymap.bind(Global, PaneForward, &["l", "right"]);
        keymap.bind(Global, LibrarySearch, &["/"]);
        keymap.bind(Global, UiDismiss, &["esc"]);
        keymap.bind(Global, ViewToggleHelp, &["?"]);
        keymap.bind(Global, LibraryRescan, &["f5", "ctrl-r"]);
        keymap.bind(Global, ViewCycleSort, &["s"]);
        keymap.bind(Global, ViewToggleSidebar, &["S"]);
        keymap.bind(Global, ViewToggleDensity, &["z"]);
        keymap.bind(Global, ViewToggleLineNumbers, &["N"]);
        keymap.bind(Global, LibraryToggleTrash, &["T"]);
        keymap.bind(Global, CopyContent, &["y"]);
        keymap.bind(Global, CopySnippetId, &["Y"]);
        keymap.bind(Global, CopyManagedPath, &["p"]);
        keymap.bind(Global, PreviewPreviousItem, &["["]);
        keymap.bind(Global, PreviewNextItem, &["]"]);
        keymap.bind(Global, PreviewPreviousParagraph, &["{"]);
        keymap.bind(Global, PreviewNextParagraph, &["}"]);
        keymap.bind(Global, GitToggleConsole, &["ctrl-g"]);
        keymap.bind(Global, GistTogglePanel, &["ctrl-s"]);

        for mode in [Sidebar, List, Preview] {
            keymap.bind(mode, NavDown, &["j", "down"]);
            keymap.bind(mode, NavUp, &["k", "up"]);
            keymap.bind(mode, NavFirst, &["g", "home"]);
            keymap.bind(mode, NavLast, &["G", "end"]);
            keymap.bind(mode, NavPageDown, &["ctrl-d"]);
            keymap.bind(mode, NavPageUp, &["ctrl-u"]);
        }
        keymap.bind(Sidebar, SidebarActivate, &["enter"]);
        keymap.bind(Sidebar, SidebarToggleFolder, &["space"]);
        keymap.bind(Sidebar, FolderNew, &["n"]);
        keymap.bind(Sidebar, SidebarRename, &["r"]);
        keymap.bind(Sidebar, FolderMove, &["m"]);
        keymap.bind(Sidebar, SidebarDelete, &["d"]);

        keymap.bind(List, ListEnterPreview, &["enter"]);
        bind_snippet_actions(&mut keymap, List);
        bind_snippet_actions(&mut keymap, Preview);
        keymap.bind(Preview, PreviewExpandFragments, &["="]);
        keymap.bind(Preview, PreviewCollapseFragments, &["-"]);

        keymap.bind(Fragment, FragmentAdd, &["n"]);
        keymap.bind(Fragment, FragmentRename, &["r"]);
        keymap.bind(Fragment, FragmentReorder, &["m"]);
        keymap.bind(Fragment, FragmentRemove, &["d"]);

        keymap.bind(FragmentGrab, NavDown, &["j", "down"]);
        keymap.bind(FragmentGrab, NavUp, &["k", "up"]);
        keymap.bind(FragmentGrab, GrabDrop, &["enter"]);
        keymap.bind(FragmentGrab, UiDismiss, &["esc", "-"]);

        keymap.bind(Trash, NavDown, &["j", "down"]);
        keymap.bind(Trash, NavUp, &["k", "up"]);
        keymap.bind(Trash, NavFirst, &["g", "home"]);
        keymap.bind(Trash, NavLast, &["G", "end"]);
        keymap.bind(Trash, NavPageDown, &["ctrl-d"]);
        keymap.bind(Trash, NavPageUp, &["ctrl-u"]);
        keymap.bind(Trash, TrashRestoreSelected, &["enter", "u"]);
        keymap.bind(Trash, TrashPurgeSelected, &["x"]);
        keymap.bind(Trash, UiDismiss, &["esc", "q"]);

        keymap.bind(Help, NavDown, &["j", "down"]);
        keymap.bind(Help, NavUp, &["k", "up"]);
        keymap.bind(Help, NavPageDown, &["ctrl-d"]);
        keymap.bind(Help, NavPageUp, &["ctrl-u"]);
        keymap.bind(Help, UiDismiss, &["esc", "q"]);

        keymap.bind(Git, GitRefreshLocalStatus, &["r"]);
        keymap.bind(Git, GitBackup, &["b"]);
        keymap.bind(Git, GitCommit, &["c"]);
        keymap.bind(Git, GitPush, &["p"]);
        keymap.bind(Git, GitFetchRemoteStatus, &["f"]);
        keymap.bind(Git, GitPull, &["l"]);
        keymap.bind(Git, GitCommitWithMessage, &["C"]);
        keymap.bind(Git, GitPauseAutoBackup, &["a"]);
        keymap.bind(Git, GitToggleAutoPush, &["u"]);
        keymap.bind(Git, GitToggleAutoPull, &["U"]);
        keymap.bind(Git, GitToggleBackupOnQuit, &["o"]);
        keymap.bind(Git, GitInitOrSetInterval, &["i"]);
        keymap.bind(Git, UiDismiss, &["esc"]);

        keymap.bind(Gist, GistPush, &["p"]);
        keymap.bind(Gist, GistPushPublic, &["P"]);
        keymap.bind(Gist, GistCopyUrl, &["y"]);
        keymap.bind(Gist, GistOpenInBrowser, &["o"]);
        keymap.bind(Gist, GistAttach, &["a"]);
        keymap.bind(Gist, GistDetach, &["d"]);
        keymap.bind(Gist, GistDelete, &["x"]);
        keymap.bind(Gist, GistVerifyRemote, &["r"]);
        keymap.bind(Gist, UiDismiss, &["esc"]);

        keymap
    }

    pub fn resolve(&self, stack: &[Mode], chord: Chord) -> Option<CommandId> {
        self.resolve_with_mode(stack, chord)
            .map(|(_, command)| command)
    }

    pub fn resolve_with_mode(&self, stack: &[Mode], chord: Chord) -> Option<(Mode, CommandId)> {
        for &mode in stack {
            if let Some(id) = self
                .modes
                .get(&mode)
                .and_then(|bindings| bindings.get(&chord))
            {
                return Some((mode, *id));
            }
            if mode.is_exclusive() {
                let inherited = self
                    .modes
                    .get(&Mode::Global)
                    .and_then(|bindings| bindings.get(&chord))
                    .filter(|id| mode.inherits().contains(id));
                return inherited.map(|id| (Mode::Global, *id));
            }
        }
        None
    }

    pub fn bindings_for(&self, mode: Mode) -> impl Iterator<Item = (Chord, CommandId)> + '_ {
        self.modes
            .get(&mode)
            .into_iter()
            .flat_map(|bindings| bindings.iter())
            .map(|(chord, id)| (*chord, *id))
    }

    pub fn chords_for(&self, modes: &[Mode], id: CommandId) -> Vec<Chord> {
        let mut chords = modes
            .iter()
            .flat_map(|mode| self.bindings_for(*mode))
            .filter_map(|(chord, bound)| (bound == id).then_some(chord))
            .collect::<Vec<_>>();
        chords.sort_unstable_by_key(|chord| chord_sort_key(*chord));
        chords.dedup();
        chords
    }

    fn bind(&mut self, mode: Mode, id: CommandId, chords: &[&str]) {
        let bindings = self.modes.entry(mode).or_default();
        for chord in chords {
            let chord = chord.parse::<Chord>().expect("built-in chords are valid");
            let previous = bindings.insert(chord, id);
            assert!(
                previous.is_none(),
                "duplicate built-in chord {chord} in {mode:?}"
            );
        }
    }
}

fn chord_sort_key(chord: Chord) -> (u8, String) {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    let rank = match (chord.modifiers() == KeyModifiers::NONE, chord.code()) {
        (true, KeyCode::Char(_)) => 0,
        (true, _) => 1,
        (false, _) => 2,
    };
    (rank, chord.canonical())
}

impl Mode {
    pub fn config_name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Sidebar => "sidebar",
            Self::List => "list",
            Self::Preview => "preview",
            Self::Fragment => "fragment",
            Self::FragmentGrab => "fragment-grab",
            Self::Trash => "trash",
            Self::Help => "help",
            Self::Git => "git",
            Self::Gist => "gist",
            Self::Search => "search",
        }
    }

    pub fn from_config_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.config_name() == name)
    }
}

fn bind_snippet_actions(keymap: &mut Keymap, mode: Mode) {
    use CommandId::*;
    keymap.bind(mode, SnippetNew, &["n"]);
    keymap.bind(mode, SnippetEditContent, &["e"]);
    keymap.bind(mode, SnippetEditNote, &["E"]);
    keymap.bind(mode, SnippetEditReadme, &["R"]);
    keymap.bind(mode, SnippetOpenVsCode, &["v"]);
    keymap.bind(mode, SnippetRename, &["r"]);
    keymap.bind(mode, SnippetMove, &["m"]);
    keymap.bind(mode, SnippetEditTags, &["t"]);
    keymap.bind(mode, SnippetEditLanguage, &["f"]);
    keymap.bind(mode, SnippetTogglePin, &["P"]);
    keymap.bind(mode, SnippetToggleLock, &["L"]);
    keymap.bind(mode, SnippetMoveToTrash, &["d"]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_modes_only_inherit_their_allowlist() {
        let keymap = Keymap::defaults();
        assert_eq!(
            keymap.resolve(&[Mode::Git], "ctrl-s".parse().unwrap()),
            Some(CommandId::GistTogglePanel)
        );
        assert_eq!(
            keymap.resolve(&[Mode::Help], "ctrl-g".parse().unwrap()),
            Some(CommandId::GitToggleConsole)
        );
        assert_eq!(
            keymap.resolve(&[Mode::Trash], "ctrl-g".parse().unwrap()),
            Some(CommandId::GitToggleConsole)
        );
        assert_eq!(keymap.resolve(&[Mode::Git], "q".parse().unwrap()), None);
    }

    #[test]
    fn resolution_reports_the_mode_that_supplied_the_binding() {
        let keymap = Keymap::defaults();

        assert_eq!(
            keymap.resolve_with_mode(&[Mode::List, Mode::Global], "e".parse().unwrap()),
            Some((Mode::List, CommandId::SnippetEditContent))
        );
        assert_eq!(
            keymap.resolve_with_mode(&[Mode::Git], "ctrl-s".parse().unwrap()),
            Some((Mode::Global, CommandId::GistTogglePanel))
        );
    }

    #[test]
    fn search_only_inherits_the_two_panel_toggles() {
        let keymap = Keymap::defaults();
        assert_eq!(
            keymap.resolve(&[Mode::Search], "ctrl-g".parse().unwrap()),
            Some(CommandId::GitToggleConsole)
        );
        assert_eq!(
            keymap.resolve(&[Mode::Search], "ctrl-s".parse().unwrap()),
            Some(CommandId::GistTogglePanel)
        );
        // Everything else belongs to the query editor, `esc` and `:` included.
        for chord in ["esc", ":", "q", "/", "j"] {
            assert_eq!(
                keymap.resolve(&[Mode::Search], chord.parse().unwrap()),
                None,
                "{chord:?} must reach the search editor"
            );
        }
    }

    #[test]
    fn action_binding_queries_are_stable_and_deduplicated() {
        let keymap = Keymap::defaults();

        assert_eq!(
            keymap
                .chords_for(&[Mode::List, Mode::Preview], CommandId::SnippetEditContent)
                .into_iter()
                .map(Chord::canonical)
                .collect::<Vec<_>>(),
            ["e"]
        );
    }

    #[test]
    fn action_chords_sort_plain_characters_before_named_and_modified_keys() {
        let keymap = Keymap::defaults();
        assert_eq!(
            keymap
                .chords_for(&[Mode::Global], CommandId::PaletteOpen)
                .into_iter()
                .map(Chord::canonical)
                .collect::<Vec<_>>(),
            [":", "ctrl-p"]
        );
        assert_eq!(
            keymap
                .chords_for(&[Mode::Global], CommandId::PaneBack)
                .into_iter()
                .map(Chord::canonical)
                .collect::<Vec<_>>(),
            ["h", "left"]
        );
    }
}
