use ratatui::crossterm::event::KeyEvent;

use crate::config::AppConfig;
use crate::filesystem::Library;
use crate::keys::{Chord, Keymap, Mode};
use crate::tui::app::types::FragmentGrab;
use crate::tui::command::CommandId;
use crate::tui::state::Pane;

use super::App;

#[derive(Clone, Copy, Debug)]
enum TestMode {
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

impl TestMode {
    const ALL: [Self; 11] = [
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

    fn keymap_mode(self) -> Mode {
        match self {
            Self::Global => Mode::Global,
            Self::Sidebar => Mode::Sidebar,
            Self::List => Mode::List,
            Self::Preview => Mode::Preview,
            Self::Fragment => Mode::Fragment,
            Self::FragmentGrab => Mode::FragmentGrab,
            Self::Trash => Mode::Trash,
            Self::Help => Mode::Help,
            Self::Git => Mode::Git,
            Self::Gist => Mode::Gist,
            Self::Search => Mode::Search,
        }
    }
}

fn app() -> (tempfile::TempDir, App) {
    let temporary = tempfile::tempdir().unwrap();
    let library = Library::init(&temporary.path().join("Keys.sniplib"), None).unwrap();
    let app = App::new(library, &AppConfig::default()).unwrap();
    (temporary, app)
}

fn prepare(app: &mut App, mode: TestMode) {
    app.focus = Pane::List;
    app.fragments_expanded = false;
    app.fragment_grab = None;
    app.trash.open = false;
    app.show_help = false;
    app.git.open = false;
    app.git.operation_queued = false;
    app.gist.open = false;
    app.search.active = false;
    app.palette.close();
    app.modal = None;
    app.status = None;
    app.should_quit = false;
    app.pending_quit = false;
    app.last_command = None;

    match mode {
        TestMode::Global | TestMode::List => {}
        TestMode::Sidebar => app.focus = Pane::Sidebar,
        TestMode::Preview => app.focus = Pane::Preview,
        TestMode::Fragment => {
            app.focus = Pane::Preview;
            app.fragments_expanded = true;
        }
        TestMode::FragmentGrab => {
            app.focus = Pane::Preview;
            app.fragment_grab = Some(FragmentGrab {
                origin: 0,
                current: 0,
            });
        }
        TestMode::Trash => {
            app.focus = Pane::List;
            app.trash.open = true;
        }
        TestMode::Help => app.show_help = true,
        TestMode::Git => app.git.open = true,
        TestMode::Gist => app.gist.open = true,
        TestMode::Search => app.search.active = true,
    }
}

fn send(app: &mut App, chord: &str) {
    let chord = chord.parse::<Chord>().unwrap();
    app.handle_key(KeyEvent::new(chord.code(), chord.modifiers()));
}

fn assert_bindings(app: &mut App, mode: TestMode, bindings: &[(&str, CommandId)]) {
    for &(chord, expected) in bindings {
        prepare(app, mode);
        send(app, chord);
        assert_eq!(
            app.last_command,
            Some(expected),
            "wrong action for {chord:?} in {mode:?}"
        );
    }
}

#[test]
fn handwritten_dispatch_matches_the_characterization_table() {
    let (_temporary, mut app) = app();
    for (mode, bindings) in characterization_table() {
        assert_bindings(&mut app, mode, &bindings);
    }
}

/// The behaviour oracle: what the handwritten dispatch did, chord by chord.
/// `every_default_binding_is_characterized` keeps it exhaustive.
fn characterization_table() -> Vec<(TestMode, Vec<(&'static str, CommandId)>)> {
    let mut table = Vec::new();

    table.push((
        TestMode::Global,
        vec![
            (":", CommandId::PaletteOpen),
            ("ctrl-p", CommandId::PaletteOpen),
            ("q", CommandId::AppQuit),
            ("tab", CommandId::PaneNext),
            ("backtab", CommandId::PanePrevious),
            ("h", CommandId::PaneBack),
            ("left", CommandId::PaneBack),
            ("l", CommandId::PaneForward),
            ("right", CommandId::PaneForward),
            ("/", CommandId::LibrarySearch),
            ("?", CommandId::ViewToggleHelp),
            ("f5", CommandId::LibraryRescan),
            ("ctrl-r", CommandId::LibraryRescan),
            ("s", CommandId::ViewCycleSort),
            ("z", CommandId::ViewToggleDensity),
            ("N", CommandId::ViewToggleLineNumbers),
            ("T", CommandId::LibraryToggleTrash),
            ("y", CommandId::CopyContent),
            ("Y", CommandId::CopySnippetId),
            ("p", CommandId::CopyManagedPath),
            ("[", CommandId::PreviewPreviousItem),
            ("]", CommandId::PreviewNextItem),
            ("{", CommandId::PreviewPreviousParagraph),
            ("}", CommandId::PreviewNextParagraph),
            ("ctrl-g", CommandId::GitToggleConsole),
            ("ctrl-s", CommandId::GistTogglePanel),
        ],
    ));

    let navigation = [
        ("j", CommandId::NavDown),
        ("down", CommandId::NavDown),
        ("k", CommandId::NavUp),
        ("up", CommandId::NavUp),
        ("g", CommandId::NavFirst),
        ("G", CommandId::NavLast),
        ("ctrl-d", CommandId::NavPageDown),
        ("ctrl-u", CommandId::NavPageUp),
    ];
    for (mode, bindings) in [
        (
            TestMode::Sidebar,
            &[
                ("enter", CommandId::SidebarActivate),
                ("space", CommandId::SidebarToggleFolder),
                ("n", CommandId::FolderNew),
                ("r", CommandId::SidebarRename),
                ("m", CommandId::FolderMove),
                ("d", CommandId::SidebarDelete),
            ][..],
        ),
        (
            TestMode::List,
            &[
                ("enter", CommandId::ListEnterPreview),
                ("n", CommandId::SnippetNew),
                ("e", CommandId::SnippetEditContent),
                ("E", CommandId::SnippetEditNote),
                ("R", CommandId::SnippetEditReadme),
                ("v", CommandId::SnippetOpenVsCode),
                ("r", CommandId::SnippetRename),
                ("m", CommandId::SnippetMove),
                ("t", CommandId::SnippetEditTags),
                ("f", CommandId::SnippetEditLanguage),
                ("P", CommandId::SnippetTogglePin),
                ("L", CommandId::SnippetToggleLock),
                ("d", CommandId::SnippetMoveToTrash),
            ][..],
        ),
        (
            TestMode::Preview,
            &[
                ("n", CommandId::SnippetNew),
                ("e", CommandId::SnippetEditContent),
                ("E", CommandId::SnippetEditNote),
                ("R", CommandId::SnippetEditReadme),
                ("v", CommandId::SnippetOpenVsCode),
                ("r", CommandId::SnippetRename),
                ("m", CommandId::SnippetMove),
                ("t", CommandId::SnippetEditTags),
                ("f", CommandId::SnippetEditLanguage),
                ("P", CommandId::SnippetTogglePin),
                ("L", CommandId::SnippetToggleLock),
                ("d", CommandId::SnippetMoveToTrash),
                ("=", CommandId::PreviewExpandFragments),
                ("-", CommandId::PreviewCollapseFragments),
            ][..],
        ),
    ] {
        table.push((mode, [&navigation[..], bindings].concat()));
    }

    table.push((
        TestMode::Fragment,
        vec![
            ("n", CommandId::FragmentAdd),
            ("r", CommandId::FragmentRename),
            ("m", CommandId::FragmentReorder),
            ("d", CommandId::FragmentRemove),
        ],
    ));
    table.push((
        TestMode::FragmentGrab,
        vec![
            ("j", CommandId::NavDown),
            ("down", CommandId::NavDown),
            ("k", CommandId::NavUp),
            ("up", CommandId::NavUp),
            ("enter", CommandId::GrabDrop),
        ],
    ));
    table.push((
        TestMode::Trash,
        vec![
            ("j", CommandId::NavDown),
            ("down", CommandId::NavDown),
            ("k", CommandId::NavUp),
            ("up", CommandId::NavUp),
            ("g", CommandId::NavFirst),
            ("home", CommandId::NavFirst),
            ("G", CommandId::NavLast),
            ("end", CommandId::NavLast),
            // Not in the handwritten dispatch: paging was help-only, and the
            // asymmetry was an oversight rather than a decision.
            ("ctrl-d", CommandId::NavPageDown),
            ("ctrl-u", CommandId::NavPageUp),
            ("enter", CommandId::TrashRestoreSelected),
            ("u", CommandId::TrashRestoreSelected),
            ("x", CommandId::TrashPurgeSelected),
        ],
    ));
    table.push((
        TestMode::Help,
        vec![
            ("j", CommandId::NavDown),
            ("down", CommandId::NavDown),
            ("k", CommandId::NavUp),
            ("up", CommandId::NavUp),
            ("ctrl-d", CommandId::NavPageDown),
            ("ctrl-u", CommandId::NavPageUp),
        ],
    ));
    table.push((
        TestMode::Git,
        vec![
            ("r", CommandId::GitRefreshLocalStatus),
            ("b", CommandId::GitBackup),
            ("c", CommandId::GitCommit),
            ("p", CommandId::GitPush),
            ("f", CommandId::GitFetchRemoteStatus),
            ("l", CommandId::GitPull),
            ("C", CommandId::GitCommitWithMessage),
            ("a", CommandId::GitPauseAutoBackup),
            ("u", CommandId::GitToggleAutoPush),
            ("U", CommandId::GitToggleAutoPull),
            ("o", CommandId::GitToggleBackupOnQuit),
            ("i", CommandId::GitInitOrSetInterval),
        ],
    ));
    table.push((
        TestMode::Gist,
        vec![
            ("p", CommandId::GistPush),
            ("P", CommandId::GistPushPublic),
            ("y", CommandId::GistCopyUrl),
            ("o", CommandId::GistOpenInBrowser),
            ("a", CommandId::GistAttach),
            ("d", CommandId::GistDetach),
            ("x", CommandId::GistDelete),
            ("r", CommandId::GistVerifyRemote),
        ],
    ));

    // Search binds nothing of its own; everything unclaimed is query text.
    table.push((TestMode::Search, Vec::new()));

    table
}

#[test]
fn handwritten_dispatch_pins_shadowing_and_inline_closers() {
    let (_temporary, mut app) = app();

    prepare(&mut app, TestMode::Git);
    send(&mut app, "q");
    assert_eq!(app.last_command, None);
    assert!(app.git.open);

    prepare(&mut app, TestMode::Gist);
    send(&mut app, "d");
    assert_eq!(app.last_command, Some(CommandId::GistDetach));

    prepare(&mut app, TestMode::FragmentGrab);
    send(&mut app, "-");
    assert!(app.fragment_grab.is_none());

    prepare(&mut app, TestMode::Trash);
    send(&mut app, "x");
    assert_eq!(app.last_command, Some(CommandId::TrashPurgeSelected));

    prepare(&mut app, TestMode::Trash);
    send(&mut app, "q");
    assert_eq!(app.last_command, Some(CommandId::UiDismiss));
    assert!(!app.should_quit);
    assert!(!app.trash.open);

    prepare(&mut app, TestMode::Help);
    send(&mut app, "q");
    assert_eq!(app.last_command, Some(CommandId::UiDismiss));
    assert!(!app.should_quit);
    assert!(!app.show_help);

    prepare(&mut app, TestMode::Trash);
    send(&mut app, "T");
    assert_eq!(app.last_command, Some(CommandId::LibraryToggleTrash));
    assert!(!app.trash.open);

    prepare(&mut app, TestMode::Git);
    send(&mut app, "ctrl-g");
    assert_eq!(app.last_command, Some(CommandId::GitToggleConsole));
    assert!(!app.git.open);

    prepare(&mut app, TestMode::Gist);
    send(&mut app, "ctrl-s");
    assert_eq!(app.last_command, Some(CommandId::GistTogglePanel));
    assert!(!app.gist.open);

    prepare(&mut app, TestMode::Help);
    send(&mut app, "?");
    assert_eq!(app.last_command, Some(CommandId::ViewToggleHelp));
    assert!(!app.show_help);
}

#[test]
fn escape_dismisses_whatever_owns_the_keyboard() {
    let (_temporary, mut app) = app();

    for mode in [
        TestMode::Global,
        TestMode::Sidebar,
        TestMode::List,
        TestMode::Preview,
        TestMode::Fragment,
        TestMode::FragmentGrab,
        TestMode::Trash,
        TestMode::Help,
        TestMode::Git,
        TestMode::Gist,
    ] {
        prepare(&mut app, mode);
        send(&mut app, "esc");
        assert_eq!(app.last_command, Some(CommandId::UiDismiss), "{mode:?}");
    }

    prepare(&mut app, TestMode::FragmentGrab);
    send(&mut app, "esc");
    assert!(app.fragment_grab.is_none());

    prepare(&mut app, TestMode::Trash);
    send(&mut app, "esc");
    assert!(!app.trash.open);

    prepare(&mut app, TestMode::Help);
    send(&mut app, "esc");
    assert!(!app.show_help);

    prepare(&mut app, TestMode::Git);
    send(&mut app, "esc");
    assert!(!app.git.open);

    prepare(&mut app, TestMode::Gist);
    send(&mut app, "esc");
    assert!(!app.gist.open);

    // Search runs its own editor: `esc` leaves the query, it is not a command.
    prepare(&mut app, TestMode::Search);
    send(&mut app, "esc");
    assert_eq!(app.last_command, None);
    assert!(!app.search.active);
}

#[test]
fn search_keeps_the_panel_toggles_reachable() {
    let (_temporary, mut app) = app();

    prepare(&mut app, TestMode::Search);
    send(&mut app, "ctrl-g");
    assert_eq!(app.last_command, Some(CommandId::GitToggleConsole));
    assert!(app.git.open);

    prepare(&mut app, TestMode::Search);
    send(&mut app, "ctrl-s");
    assert_eq!(app.last_command, Some(CommandId::GistTogglePanel));
    assert!(app.gist.open);

    // Nothing else escapes the query editor, including chords the panes bind.
    for chord in ["q", "j", ":", "/", "T", "1"] {
        prepare(&mut app, TestMode::Search);
        app.search.query.clear();
        send(&mut app, chord);
        assert_eq!(app.last_command, None, "{chord:?} must stay query text");
        assert!(
            app.search.active,
            "{chord:?} must stay in the search editor"
        );
        assert_eq!(app.search.query, chord, "{chord:?} must reach the query");
    }
}

#[test]
fn every_default_binding_is_characterized() {
    use std::collections::BTreeSet;

    let keymap = Keymap::defaults();
    let mut characterized: BTreeSet<(String, String)> = characterization_table()
        .into_iter()
        .flat_map(|(mode, bindings)| {
            bindings.into_iter().map(move |(chord, _)| {
                (
                    format!("{:?}", mode.keymap_mode()),
                    chord.parse::<Chord>().unwrap().to_string(),
                )
            })
        })
        .collect();
    for (mode, chord) in characterized_elsewhere() {
        characterized.insert((
            format!("{:?}", mode.keymap_mode()),
            chord.parse::<Chord>().unwrap().to_string(),
        ));
    }

    let bound: BTreeSet<(String, String)> = TestMode::ALL
        .into_iter()
        .flat_map(|mode| {
            let mode = mode.keymap_mode();
            keymap
                .bindings_for(mode)
                .map(move |(chord, _)| (format!("{mode:?}"), chord.to_string()))
                .collect::<Vec<_>>()
        })
        .collect();

    let untested: Vec<_> = bound.difference(&characterized).collect();
    let stale: Vec<_> = characterized.difference(&bound).collect();
    assert!(
        untested.is_empty() && stale.is_empty(),
        "default bindings with no test: {untested:?}\n\
         tested chords that are no longer bound: {stale:?}"
    );
}

/// Chords the tests above pin outside the characterization table, so that
/// `every_default_binding_is_characterized` still sees full coverage.
fn characterized_elsewhere() -> Vec<(TestMode, &'static str)> {
    vec![
        // a4_intentional_behavior_changes_are_explicit
        (TestMode::Sidebar, "home"),
        (TestMode::Sidebar, "end"),
        (TestMode::List, "home"),
        (TestMode::List, "end"),
        (TestMode::Preview, "home"),
        (TestMode::Preview, "end"),
        // handwritten_dispatch_pins_shadowing_and_inline_closers
        (TestMode::FragmentGrab, "-"),
        (TestMode::Trash, "q"),
        (TestMode::Help, "q"),
        // escape_dismisses_whatever_owns_the_keyboard
        (TestMode::Global, "esc"),
        (TestMode::FragmentGrab, "esc"),
        (TestMode::Trash, "esc"),
        (TestMode::Help, "esc"),
        (TestMode::Git, "esc"),
        (TestMode::Gist, "esc"),
    ]
}

#[test]
fn a4_intentional_behavior_changes_are_explicit() {
    let (_temporary, mut app) = app();

    // A4: Home/End are navigation aliases in the three stacking pane modes.
    for mode in [TestMode::Sidebar, TestMode::List, TestMode::Preview] {
        for chord in ["home", "end"] {
            prepare(&mut app, mode);
            send(&mut app, chord);
            let expected = if chord == "home" {
                CommandId::NavFirst
            } else {
                CommandId::NavLast
            };
            assert_eq!(app.last_command, Some(expected), "{chord:?} in {mode:?}");
        }
    }

    // A4: exact modifiers stop Alt-s from falling through to plain s.
    prepare(&mut app, TestMode::Global);
    send(&mut app, "alt-s");
    assert_eq!(app.last_command, None);
}

#[test]
fn control_c_is_an_unconditional_escape_hatch() {
    let (_temporary, mut app) = app();
    for mode in [
        TestMode::Global,
        TestMode::Sidebar,
        TestMode::List,
        TestMode::Preview,
        TestMode::Fragment,
        TestMode::FragmentGrab,
        TestMode::Trash,
        TestMode::Help,
        TestMode::Git,
        TestMode::Gist,
    ] {
        prepare(&mut app, mode);
        send(&mut app, "ctrl-c");
        assert_eq!(app.last_command, Some(CommandId::AppQuit), "{mode:?}");
    }
}
