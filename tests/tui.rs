#![cfg(feature = "tui")]

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use snip::service::{CreateOptions, EditOptions, create_snippet, edit_snippet};
use snip::tui::app::{App, Effect};
use snip::tui::editor::{EditOutcome, force_save};
use snip::tui::highlight::Highlighter;
use snip::tui::state::{Pane, PendingPrompt, SidebarItem};
use snip::tui::theme::{Appearance, TuiTheme};
use snip::{AppConfig, Library};
use tempfile::TempDir;

fn fixture() -> (TempDir, Library, uuid::Uuid, uuid::Uuid) {
    let temporary = tempfile::tempdir_in(".").unwrap();
    let root = temporary.path().join("Tui.sniplib");
    let library = Library::init(&root, Some("TUI fixture")).unwrap();
    let first = create_snippet(
        &library,
        &CreateOptions {
            title: "Alpha Rust".to_owned(),
            folder: Some("Code/Rust".to_owned()),
            tags: vec!["dev".to_owned()],
            language: "rust".to_owned(),
            content: "fn alpha() {}\n".to_owned(),
            note: Some("**Rust** note".to_owned()),
            pinned: true,
            ..CreateOptions::default()
        },
    )
    .unwrap();
    let second = create_snippet(
        &library,
        &CreateOptions {
            title: "Beta Shell".to_owned(),
            folder: Some("Code/Shell".to_owned()),
            tags: vec!["ops".to_owned()],
            language: "bash".to_owned(),
            content: "echo searchable needle\n".to_owned(),
            ..CreateOptions::default()
        },
    )
    .unwrap();
    (temporary, library, first.id, second.id)
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn navigation_recursive_filter_and_search_work_headlessly() {
    let (_temporary, library, first_id, second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    assert_eq!(app.visible[0].snippet_id, first_id, "pinned snippets lead");

    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(app.focus, Pane::Sidebar);
    assert_eq!(
        app.sidebar.selected().map(|row| &row.item),
        Some(&SidebarItem::Folder("Code".to_owned()))
    );
    assert_eq!(app.visible.len(), 2, "moving applies the folder filter");

    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(app.focus, Pane::Sidebar);
    assert_eq!(
        app.sidebar.selected().map(|row| &row.item),
        Some(&SidebarItem::Folder("Code/Rust".to_owned()))
    );
    assert_eq!(app.visible.len(), 1);
    assert_eq!(app.visible[0].snippet_id, first_id);

    let code_row = app
        .sidebar
        .rows
        .iter()
        .position(|row| row.item == SidebarItem::Folder("Code".to_owned()))
        .unwrap();
    app.sidebar.list_state.select(Some(code_row));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.focus, Pane::List);
    assert_eq!(app.visible.len(), 2, "folder filters include descendants");

    app.handle_key(key(KeyCode::Char('/')));
    for ch in "needle".chars() {
        app.handle_key(key(KeyCode::Char(ch)));
    }
    assert_eq!(app.visible.len(), 1);
    assert_eq!(app.visible[0].snippet_id, second_id);
    assert_eq!(
        app.visible[0].excerpt.as_deref(),
        Some("echo searchable needle")
    );
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.focus, Pane::List);
}

#[test]
fn rescan_preserves_selection_by_uuid_after_external_change() {
    let (_temporary, library, _first_id, second_id) = fixture();
    let mut app = App::new(library.clone(), &AppConfig::default()).unwrap();
    let index = app
        .visible
        .iter()
        .position(|row| row.snippet_id == second_id)
        .unwrap();
    app.list_state.select(Some(index));
    app.selected_id = Some(second_id);
    let snippet = app.selected_snippet().unwrap().clone();

    edit_snippet(
        &library,
        &second_id.to_string(),
        &EditOptions {
            content: Some("echo externally changed\n".to_owned()),
            if_hash: Some(snippet.fingerprint),
            ..EditOptions::default()
        },
    )
    .unwrap();
    app.rescan().unwrap();
    assert_eq!(app.selected_id, Some(second_id));
    assert_eq!(
        app.selected_snippet().unwrap().loaded_fragments[0].content,
        "echo externally changed\n"
    );
}

#[test]
fn edit_effect_captures_hash_and_conflict_can_force_save() {
    let (_temporary, library, first_id, _second_id) = fixture();
    let mut app = App::new(library.clone(), &AppConfig::default()).unwrap();
    app.selected_id = Some(first_id);
    app.list_state.select(Some(
        app.visible
            .iter()
            .position(|row| row.snippet_id == first_id)
            .unwrap(),
    ));
    let original_hash = app.selected_snippet().unwrap().fingerprint.clone();
    let effects = app.handle_key(key(KeyCode::Char('e')));
    let Effect::SpawnEditor(mut request) = effects.into_iter().next().unwrap() else {
        panic!("expected editor effect");
    };
    assert_eq!(request.expected, original_hash);
    request.edited = Some("fn forced() {}\n".to_owned());

    edit_snippet(
        &library,
        &first_id.to_string(),
        &EditOptions {
            content: Some("fn agent() {}\n".to_owned()),
            if_hash: Some(original_hash),
            ..EditOptions::default()
        },
    )
    .unwrap();
    app.handle_editor_outcome(EditOutcome::Conflict(request));
    assert!(matches!(app.pending, Some(PendingPrompt::ForceEdit(_))));
    let effects = app.handle_key(key(KeyCode::Char('y')));
    let Effect::ForceSave(request) = effects.into_iter().next().unwrap() else {
        panic!("expected force-save effect");
    };
    force_save(&library, &request).unwrap();
    app.rescan().unwrap();
    assert_eq!(
        app.selected_snippet().unwrap().loaded_fragments[0].content,
        "fn forced() {}\n"
    );
}

#[test]
fn three_pane_ui_draws_titles_preview_and_status() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.theme = TuiTheme::for_appearance(Appearance::Light);
    app.highlighter = Highlighter::new(app.theme).unwrap();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Library"));
    assert!(rendered.contains("Snippets"));
    assert!(rendered.contains("Preview"));
    assert!(rendered.contains("Alpha Rust"));
    assert!(rendered.contains("[rs]"));
    assert!(rendered.contains("◆ Library"));
    assert!(rendered.contains("NORMAL"));
    assert!(rendered.contains("LIBRARY"));
    assert!(rendered.contains("PANE"));
    assert!(rendered.contains("MOVE"));
    assert!(rendered.contains("SEARCH"));
    assert!(rendered.contains("1/2"));
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "┏");
    assert_eq!(buffer.cell((24, 0)).unwrap().symbol(), "┌");
    assert_eq!(buffer.cell((3, 1)).unwrap().bg, app.theme.selection_bg);
    assert_eq!(buffer.cell((18, 28)).unwrap().bg, app.theme.bar_bg);
    assert_eq!(buffer.cell((10, 29)).unwrap().bg, app.theme.bar_bg);

    app.handle_key(key(KeyCode::Tab));
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "┌");
    assert_eq!(buffer.cell((24, 0)).unwrap().symbol(), "┏");
}

#[cfg(unix)]
#[test]
fn external_editor_command_saves_through_optimistic_service() {
    let (_temporary, library, first_id, _second_id) = fixture();
    let catalog = library.scan().unwrap();
    let snippet = library
        .resolve_snippet(&catalog, &first_id.to_string())
        .unwrap();
    let fragment = &snippet.loaded_fragments[0];
    let request = snip::tui::editor::EditRequest {
        snippet_id: snippet.id,
        fragment_id: fragment.id,
        expected: snippet.fingerprint.clone(),
        original: fragment.content.clone(),
        edited: None,
        suffix: "rs".to_owned(),
    };
    let outcome = snip::tui::editor::run_external_edit(
        &library,
        request,
        Some("sh -c 'printf \"fn editor_saved() {}\\n\" > \"$1\"' sh"),
    )
    .unwrap();
    assert!(matches!(outcome, EditOutcome::Saved));
    let saved = library.scan().unwrap();
    assert_eq!(
        library
            .resolve_snippet(&saved, &first_id.to_string())
            .unwrap()
            .loaded_fragments[0]
            .content,
        "fn editor_saved() {}\n"
    );
}
