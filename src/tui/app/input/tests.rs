use ratatui::crossterm::event::KeyEvent;

use crate::config::AppConfig;
use crate::filesystem::Library;
use crate::keys::Chord;
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

    assert_bindings(
        &mut app,
        TestMode::Global,
        &[
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
            ("T", CommandId::LibraryOpenTrash),
            ("y", CommandId::CopyContent),
            ("Y", CommandId::CopySnippetId),
            ("p", CommandId::CopyManagedPath),
            ("[", CommandId::PreviewPreviousItem),
            ("]", CommandId::PreviewNextItem),
            ("{", CommandId::PreviewPreviousParagraph),
            ("}", CommandId::PreviewNextParagraph),
            ("ctrl-g", CommandId::GitOpenConsole),
            ("ctrl-s", CommandId::GistOpenPanel),
        ],
    );

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
        assert_bindings(&mut app, mode, &navigation);
        assert_bindings(&mut app, mode, bindings);
    }

    assert_bindings(
        &mut app,
        TestMode::Fragment,
        &[
            ("n", CommandId::FragmentAdd),
            ("r", CommandId::FragmentRename),
            ("m", CommandId::FragmentReorder),
            ("d", CommandId::FragmentRemove),
        ],
    );
    assert_bindings(
        &mut app,
        TestMode::FragmentGrab,
        &[
            ("j", CommandId::NavDown),
            ("down", CommandId::NavDown),
            ("k", CommandId::NavUp),
            ("up", CommandId::NavUp),
            ("enter", CommandId::GrabDrop),
        ],
    );
    assert_bindings(
        &mut app,
        TestMode::Trash,
        &[
            ("j", CommandId::NavDown),
            ("down", CommandId::NavDown),
            ("k", CommandId::NavUp),
            ("up", CommandId::NavUp),
            ("g", CommandId::NavFirst),
            ("home", CommandId::NavFirst),
            ("G", CommandId::NavLast),
            ("end", CommandId::NavLast),
            ("enter", CommandId::TrashRestoreSelected),
            ("u", CommandId::TrashRestoreSelected),
            ("x", CommandId::TrashPurgeSelected),
        ],
    );
    assert_bindings(
        &mut app,
        TestMode::Help,
        &[
            ("j", CommandId::NavDown),
            ("down", CommandId::NavDown),
            ("k", CommandId::NavUp),
            ("up", CommandId::NavUp),
            ("ctrl-d", CommandId::NavPageDown),
            ("ctrl-u", CommandId::NavPageUp),
        ],
    );
    assert_bindings(
        &mut app,
        TestMode::Git,
        &[
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
    );
    assert_bindings(
        &mut app,
        TestMode::Gist,
        &[
            ("p", CommandId::GistPush),
            ("P", CommandId::GistPushPublic),
            ("y", CommandId::GistCopyUrl),
            ("o", CommandId::GistOpenInBrowser),
            ("a", CommandId::GistAttach),
            ("d", CommandId::GistDetach),
            ("x", CommandId::GistDelete),
            ("r", CommandId::GistVerifyRemote),
        ],
    );
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
    assert!(app.should_quit);
    assert!(app.trash.open);

    prepare(&mut app, TestMode::Help);
    send(&mut app, "q");
    assert!(app.should_quit);
    assert!(app.show_help);

    prepare(&mut app, TestMode::Trash);
    send(&mut app, "T");
    assert!(!app.trash.open);

    prepare(&mut app, TestMode::Git);
    send(&mut app, "ctrl-g");
    assert!(!app.git.open);

    prepare(&mut app, TestMode::Gist);
    send(&mut app, "ctrl-s");
    assert!(!app.gist.open);

    prepare(&mut app, TestMode::Help);
    send(&mut app, "?");
    assert!(!app.show_help);
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
