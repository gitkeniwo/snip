#![cfg(feature = "tui")]

use std::process::Command as ProcessCommand;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use snip::git::GitAction;
use snip::git::{Branch, RepoState, Status, Unavailable};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, add_fragment, create_snippet, edit_snippet,
};
use snip::tui::app::{App, Effect};
use snip::tui::command::CommandId;
use snip::tui::editor::{EditOutcome, EditTarget, force_save};
use snip::tui::event::AppEvent;
use snip::tui::highlight::Highlighter;
use snip::tui::icons::IconMode;
use snip::tui::modal::{InputModal, Modal, ModalAction};
use snip::tui::state::{Pane, SidebarItem, SortMode};
use snip::tui::theme::{Appearance, TuiTheme};
use snip::{AppConfig, GitConfig, Library, TuiConfig, TuiDensitySetting, TuiThemeSetting};
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

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn git_available() -> bool {
    ProcessCommand::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn git_ok(path: &std::path::Path, arguments: &[&str]) {
    let status = ProcessCommand::new("git")
        .args(["-c", "init.defaultBranch=main"])
        .args(arguments)
        .current_dir(path)
        .env("GIT_CONFIG_GLOBAL", path.join(".empty-gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .unwrap();
    assert!(status.success(), "git {} failed", arguments.join(" "));
}

fn init_git_repo(path: &std::path::Path) {
    git_ok(path, &["init"]);
    git_ok(path, &["config", "user.name", "snip CI"]);
    git_ok(path, &["config", "user.email", "ci@example.invalid"]);
}

fn row_text(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer.cell((x, y)).unwrap().symbol())
        .collect()
}

fn row_text_from(buffer: &ratatui::buffer::Buffer, y: u16, x_start: u16) -> String {
    (x_start..buffer.area.width)
        .map(|x| buffer.cell((x, y)).unwrap().symbol())
        .collect()
}

fn text_x(buffer: &ratatui::buffer::Buffer, y: u16, needle: &str) -> Option<u16> {
    let row = row_text(buffer, y).chars().collect::<Vec<_>>();
    let needle = needle.chars().collect::<Vec<_>>();
    row.windows(needle.len())
        .position(|window| window == needle)
        .map(|index| index as u16)
}

fn text_column(value: &str, needle: &str) -> u16 {
    let byte_index = value.find(needle).expect("text should contain needle");
    value[..byte_index].chars().count() as u16
}

fn text_column_from_end(value: &str, needle: &str) -> u16 {
    let byte_index = value.rfind(needle).expect("text should contain needle");
    value[..byte_index].chars().count() as u16
}

fn replace_modal_input(app: &mut App, value: &str) {
    let Some(Modal::Input(input)) = app.modal.as_mut() else {
        panic!("expected input modal");
    };
    input.value = value.to_owned();
    input.cursor = value.chars().count();
}

fn select_sidebar_item(app: &mut App, item: SidebarItem) {
    let index = app
        .sidebar
        .rows
        .iter()
        .position(|row| row.item == item)
        .unwrap();
    app.sidebar.list_state.select(Some(index));
    app.focus = Pane::Sidebar;
}

#[test]
fn command_palette_fuzzy_match_and_recent_order() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.palette.open();
    app.refresh_palette();
    app.palette.input.value = "gp".to_owned();
    app.palette.input.cursor = 2;
    app.refresh_palette();
    assert!(
        app.palette
            .matches
            .iter()
            .take(3)
            .any(|matched| matched.id == CommandId::GitPush)
    );
    app.palette.input.value = "commit".to_owned();
    app.palette.input.cursor = 6;
    app.refresh_palette();
    let commit = app
        .palette
        .matches
        .iter()
        .position(|matched| matched.id == CommandId::GitCommit)
        .unwrap();
    let message = app
        .palette
        .matches
        .iter()
        .position(|matched| matched.id == CommandId::GitCommitWithMessage)
        .unwrap();
    assert!(commit < message);
    app.palette.input.value = "push".to_owned();
    app.palette.input.cursor = 4;
    app.refresh_palette();
    assert_eq!(app.palette.matches[0].id, CommandId::GitPush);
    app.run_command(CommandId::ViewCycleSort);
    app.run_command(CommandId::ViewToggleDensity);
    app.palette.open();
    app.refresh_palette();
    assert_eq!(app.palette.matches[0].id, CommandId::ViewToggleDensity);
    assert_eq!(app.palette.matches[1].id, CommandId::ViewCycleSort);
}

#[test]
fn command_palette_disabled_commands_report_status() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.selected_id = None;
    assert!(app.run_command(CommandId::SnippetEditContent).is_empty());
    assert_eq!(
        app.status.as_ref().unwrap().level,
        snip::tui::state::StatusLevel::Error
    );
    assert_eq!(app.status.as_ref().unwrap().text, "no snippet selected");
}

#[test]
fn command_palette_handles_input_and_executes_selected_command() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.handle_key(key(KeyCode::Char(':')));
    assert!(app.palette.open);
    for character in "copy id".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    assert_eq!(app.palette.matches[0].id, CommandId::CopySnippetId);
    let effects = app.handle_key(key(KeyCode::Enter));
    assert!(!app.palette.open);
    assert!(matches!(
        effects.as_slice(),
        [Effect::CopyToClipboard { .. }]
    ));
}

#[test]
fn command_palette_resets_selection_when_the_query_changes() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.handle_key(key(KeyCode::Char(':')));
    for character in "git".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    for _ in 0..3 {
        app.handle_key(key(KeyCode::Down));
    }
    assert_eq!(app.palette.selected, 3);
    app.handle_key(key(KeyCode::Char('p')));
    assert_eq!(app.palette.selected, 0);
    assert_eq!(app.palette.scroll, 0);
    assert_eq!(app.palette.matches[0].id, CommandId::GitPush);
}

#[test]
fn command_palette_opens_over_help_and_filters_hidden_commands() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.handle_key(key(KeyCode::Char('?')));
    assert!(app.show_help);
    app.handle_key(key(KeyCode::Char(':')));
    assert!(app.palette.open);

    let hidden = std::collections::HashSet::from([CommandId::GitPush]);
    app.palette.refresh(&hidden);
    assert!(
        !app.palette
            .matches
            .iter()
            .any(|matched| matched.id == CommandId::GitPush)
    );
}

#[test]
fn command_palette_uses_the_rendered_viewport_on_short_terminals() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.handle_key(key(KeyCode::Char(':')));
    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert_eq!(app.palette.visible_rows, 7);
    for _ in 0..8 {
        app.handle_key(key(KeyCode::Down));
    }
    assert_eq!(app.palette.selected, 8);
    assert_eq!(app.palette.scroll, 2);
    assert!(
        (app.palette.scroll..app.palette.scroll + app.palette.visible_rows)
            .contains(&app.palette.selected)
    );
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(
        row_text(buffer, 11).contains("←/→"),
        "palette must not clear the bottom bar"
    );
}

#[test]
fn command_palette_reclamps_scroll_after_a_terminal_resize() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.handle_key(key(KeyCode::Char(':')));
    let mut large = Terminal::new(TestBackend::new(80, 40)).unwrap();
    large
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert_eq!(app.palette.visible_rows, 10);
    for _ in 0..9 {
        app.handle_key(key(KeyCode::Down));
    }
    assert_eq!((app.palette.selected, app.palette.scroll), (9, 0));

    let mut small = Terminal::new(TestBackend::new(60, 10)).unwrap();
    small
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert_eq!(app.palette.visible_rows, 5);
    assert_eq!(app.palette.scroll, 5);
    assert!(
        (app.palette.scroll..app.palette.scroll + app.palette.visible_rows)
            .contains(&app.palette.selected)
    );
}

#[test]
fn command_palette_renders_one_result_on_an_extremely_short_terminal() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.handle_key(key(KeyCode::Char(':')));
    let mut terminal = Terminal::new(TestBackend::new(40, 5)).unwrap();
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert_eq!(app.palette.visible_rows, 1);
    let rendered = (0..5)
        .map(|row| row_text(terminal.backend().buffer(), row))
        .collect::<String>();
    assert!(rendered.contains("New Snippet"));
}

#[test]
fn git_commands_explain_when_the_git_binary_is_missing() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.git.repo = None;
    app.git.unavailable = Some(Unavailable::BinaryMissing);
    assert!(app.run_command(CommandId::GitPush).is_empty());
    assert_eq!(app.status.as_ref().unwrap().text, "git not found in PATH");
}

#[test]
fn git_commands_explain_when_repository_probe_fails() {
    let (_temporary, library, _first, _second) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.git.repo = None;
    app.git.unavailable = Some(Unavailable::ProbeFailed {
        message: "permission denied".to_owned(),
    });
    assert!(app.run_command(CommandId::GitPush).is_empty());
    assert_eq!(
        app.status.as_ref().unwrap().text,
        "git could not inspect this repository"
    );
}

#[test]
fn navigation_recursive_filter_and_search_work_headlessly() {
    let (_temporary, library, first_id, second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    assert_eq!(app.visible[0].snippet_id, first_id, "pinned snippets lead");

    let code_row = app
        .sidebar
        .rows
        .iter()
        .position(|row| row.item == SidebarItem::Folder("Code".to_owned()))
        .unwrap();
    app.sidebar.list_state.select(Some(code_row));
    app.handle_key(key(KeyCode::Enter));
    app.focus = Pane::Sidebar;
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
    app.focus = Pane::List;
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
    assert!(matches!(
        app.modal,
        Some(Modal::Confirm(ref modal)) if matches!(modal.action, ModalAction::ForceEdit(_))
    ));
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
    assert!(rendered.contains("snip"));
    assert!(rendered.contains("~ › All snippets"));
    assert!(rendered.contains("↓ modified"));
    assert!(rendered.contains("#1/2"));
    assert!(rendered.contains("fragment 1/1"));
    assert!(rendered.contains("←/→"));
    assert!(rendered.contains('/'));
    assert!(rendered.contains("001-Alpha Rust.rs rs"));
    assert!(rendered.contains("1│ fn alpha() {}"));
    let buffer = terminal.backend().buffer();
    let bottom = row_text(buffer, 29);
    assert!(bottom.starts_with('\u{e0b6}'));
    assert!(bottom.ends_with('\u{e0b4}'));
    assert!(bottom.find("←/→").unwrap() < 10);
    assert!(
        bottom.rfind("create").unwrap() > 60,
        "pane-specific actions should be grouped on the right"
    );
    let nav_key_x = text_column(&bottom, "←/→");
    assert_eq!(
        buffer.cell((nav_key_x, 29)).unwrap().bg,
        app.theme.pill_primary
    );
    let nav_join_x = nav_key_x + 3;
    assert_eq!(buffer.cell((0, 29)).unwrap().fg, app.theme.pill_primary);
    assert_eq!(buffer.cell((0, 29)).unwrap().bg, app.theme.bar_bg);
    assert_eq!(buffer.cell((nav_join_x, 29)).unwrap().symbol(), "\u{e0b4}");
    assert_eq!(
        buffer.cell((nav_join_x, 29)).unwrap().fg,
        app.theme.pill_primary
    );
    assert_eq!(
        buffer.cell((nav_join_x, 29)).unwrap().bg,
        app.theme.pill_secondary
    );
    let action_x = text_column_from_end(&bottom, "create");
    assert_eq!(
        buffer.cell((action_x, 29)).unwrap().bg,
        app.theme.pill_secondary
    );
    let top = row_text(buffer, 0);
    assert!(top.starts_with('\u{e0b6}'));
    assert!(top.ends_with('\u{e0b4}'));
    let brand_x = text_column(&top, "snip");
    let breadcrumb_x = text_column(&top, "~");
    let counts_x = text_column_from_end(&top, "#1/2");
    assert_eq!(
        buffer.cell((brand_x, 0)).unwrap().bg,
        app.theme.pill_primary
    );
    assert_eq!(
        buffer.cell((breadcrumb_x, 0)).unwrap().bg,
        app.theme.pill_secondary
    );
    assert_eq!(
        buffer.cell((counts_x, 0)).unwrap().bg,
        app.theme.pill_primary
    );
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, app.theme.pill_primary);
    assert_eq!(buffer.cell((0, 0)).unwrap().bg, app.theme.bar_bg);
    let brand_join_x = brand_x + 5;
    assert_eq!(buffer.cell((brand_join_x, 0)).unwrap().symbol(), "\u{e0b4}");
    assert_eq!(
        buffer.cell((brand_join_x, 0)).unwrap().fg,
        app.theme.pill_primary
    );
    assert_eq!(
        buffer.cell((brand_join_x, 0)).unwrap().bg,
        app.theme.pill_secondary
    );
    assert_eq!(buffer.cell((99, 0)).unwrap().fg, app.theme.pill_primary);
    assert_eq!(buffer.cell((99, 0)).unwrap().bg, app.theme.bar_bg);
    assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), "╭");
    assert_eq!(buffer.cell((24, 1)).unwrap().symbol(), "╭");
    assert_eq!(buffer.cell((0, 1)).unwrap().fg, app.theme.accent);
    assert_eq!(buffer.cell((24, 1)).unwrap().fg, app.theme.border);
    assert_eq!(buffer.cell((3, 2)).unwrap().bg, app.theme.selection_bg);
    assert_eq!(buffer.cell((26, 1)).unwrap().symbol(), "S");
    assert_eq!(
        buffer.cell((26, 2)).unwrap().symbol(),
        "r",
        "the badge starts directly below the S in Snippets"
    );
    assert_eq!(buffer.cell((26, 2)).unwrap().bg, app.theme.retained_bg);
    assert_ne!(
        buffer.cell((26, 3)).unwrap().bg,
        app.theme.retained_bg,
        "a retained selection only highlights the title row"
    );
    assert_eq!(buffer.cell((30, 2)).unwrap().symbol(), "A");
    assert_eq!(buffer.cell((30, 3)).unwrap().symbol(), "[");
    assert_eq!(buffer.cell((27, 3)).unwrap().symbol(), "★");
    assert!(row_text_from(buffer, 3, 30).starts_with("[Code > Rust]"));
    assert_eq!(
        buffer.cell((2, 10)).unwrap().symbol(),
        "#",
        "top-level tags should not inherit the folder icon gutter"
    );
    assert_eq!(buffer.cell((2, 1)).unwrap().symbol(), "L");
    assert_eq!(buffer.cell((2, 5)).unwrap().symbol(), "▾");
    assert_eq!(buffer.cell((2, 9)).unwrap().symbol(), "T");
    assert_eq!(buffer.cell((3, 9)).unwrap().symbol(), "a");
    assert_eq!(buffer.cell((2, 10)).unwrap().symbol(), "#");
    assert_eq!(buffer.cell((3, 10)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((56, 1)).unwrap().symbol(), "P");
    assert_eq!(buffer.cell((56, 2)).unwrap().symbol(), "A");
    assert_eq!(buffer.cell((56, 3)).unwrap().symbol(), "C");
    assert_eq!(buffer.cell((56, 4)).unwrap().symbol(), "f");
    assert_eq!(buffer.cell((56, 5)).unwrap().symbol(), "#");
    let preview_bottom = row_text_from(buffer, 28, 54);
    assert!(preview_bottom.contains("Rust"));
    assert!(preview_bottom.contains("1 line"));

    let metadata = row_text_from(buffer, 3, 54);
    let fragment = row_text_from(buffer, 4, 54);
    let tags = row_text_from(buffer, 5, 54);
    assert!(metadata.contains("Code/Rust · "));
    assert!(!metadata.contains("#dev"));
    assert!(fragment.contains("fragment 1/1 · "));
    assert!(fragment.contains("001-Alpha Rust.rs rs"));
    assert!(
        tags.contains("#dev"),
        "preview tags belong on their own row"
    );
    let filename_x = 54 + fragment.find("001-Alpha Rust.rs rs").unwrap() as u16;
    assert_ne!(
        buffer.cell((filename_x, 4)).unwrap().bg,
        app.theme.selection_bg,
        "the active filename should not use a filled selection chip"
    );
    assert!(row_text_from(buffer, 2, 54).contains("★ pinned"));
    let preview_start = app.layout.preview_content.y;
    let note_y = (preview_start..preview_start + 12)
        .find(|&y| row_text(buffer, y).contains("Note"))
        .expect("fixture note header should be visible");
    let note_content_y = (note_y + 1..preview_start + 12)
        .find(|&y| row_text(buffer, y).contains("Rust note"))
        .expect("fixture note content should be visible");
    assert_eq!(buffer.cell((56, note_y)).unwrap().symbol(), "N");
    assert_eq!(buffer.cell((56, note_content_y)).unwrap().symbol(), "R");
    assert_eq!(
        buffer.cell((55, note_y)).unwrap().symbol(),
        " ",
        "note prose should align with the Preview title, not the code gutter"
    );
    let preview = (preview_start..preview_start + 12)
        .map(|y| row_text(buffer, y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(preview.contains("Note"));
    assert!(!preview.contains("Note  ─"));
    assert!(preview.find("Rust note").unwrap() < preview.find("fn alpha() {}").unwrap());

    app.handle_key(key(KeyCode::Tab));
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 1)).unwrap().fg, app.theme.border);
    assert_eq!(buffer.cell((24, 1)).unwrap().fg, app.theme.accent);
    assert_eq!(buffer.cell((25, 2)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((25, 2)).unwrap().bg, app.theme.selection_bg);
    assert_eq!(buffer.cell((26, 3)).unwrap().bg, app.theme.selection_bg);

    app.handle_key(key(KeyCode::Char('N')));
    assert!(!app.show_line_numbers);
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
    assert!(!rendered.contains("1│ fn alpha() {}"));
    assert!(rendered.contains("fn alpha() {}"));
    let buffer = terminal.backend().buffer();
    let code_y = app.layout.preview_content.y + 3;
    assert_eq!(app.layout.preview_content.x, 56);
    assert_eq!(buffer.cell((55, code_y)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((56, code_y)).unwrap().symbol(), "f");

    app.handle_key(key(KeyCode::Char('?')));
    let backend = TestBackend::new(120, 42);
    let mut help_terminal = Terminal::new(backend).unwrap();
    help_terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    let buffer = help_terminal.backend().buffer();
    let rendered = buffer
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("MOVE — ALL PANES"));
    assert!(rendered.contains("SIDEBAR — WHEN THE LEFT PANE HAS FOCUS"));
    assert!(rendered.contains("SNIPPETS — WHEN LIST OR PREVIEW HAS FOCUS"));
    assert!(rendered.contains("GIT CONSOLE — WHEN OPEN"));
    for label in [
        "Help",
        "snip TUI",
        "MOVE — ALL PANES",
        "SIDEBAR — WHEN THE LEFT PANE HAS FOCUS",
        "SNIPPETS — WHEN LIST OR PREVIEW HAS FOCUS",
        "VIEW & GLOBAL",
        "GIT CONSOLE — WHEN OPEN",
    ] {
        let (y, x) = (0..buffer.area.height)
            .find_map(|y| text_x(buffer, y, label).map(|x| (y, x)))
            .unwrap_or_else(|| panic!("missing centered help label: {label}"));
        let center = x + label.chars().count() as u16 / 2;
        assert!(
            center.abs_diff(buffer.area.width / 2) <= 1,
            "{label} is not centered on row {y}"
        );
    }
    let rows = (0..buffer.area.height)
        .map(|y| row_text(buffer, y))
        .collect::<Vec<_>>();
    assert!(rows.iter().all(|row| !row.contains("g / G")));
    assert!(rows.iter().all(|row| !row.contains("r / m / t")));
    assert!(rows.iter().all(|row| !row.contains("e / E / R")));
    assert!(rendered.contains("first / last item"));
    assert!(rendered.contains("last item"));
    assert!(rendered.contains("rename snippet"));
    assert!(rendered.contains("move snippet"));
    assert!(rendered.contains("edit tags"));
    let tab_y = (0..buffer.area.height)
        .find(|&y| text_x(buffer, y, "Tab / Shift-Tab").is_some())
        .unwrap();
    assert!(row_text(buffer, tab_y).contains("next / previous"));
    assert!(rendered.contains("Ctrl-d / Ctrl-u"));
}

#[test]
fn preview_omits_the_tags_row_when_a_snippet_has_no_tags() {
    let (_temporary, library, _first_id, second_id) = fixture();
    let catalog = library.scan().unwrap();
    let snippet = library
        .resolve_snippet(&catalog, &second_id.to_string())
        .unwrap();
    edit_snippet(
        &library,
        &second_id.to_string(),
        &EditOptions {
            tags: Some(Vec::new()),
            if_hash: Some(snippet.fingerprint.clone()),
            ..EditOptions::default()
        },
    )
    .unwrap();

    let mut app = App::new(library, &AppConfig::default()).unwrap();
    let index = app
        .visible
        .iter()
        .position(|row| row.snippet_id == second_id)
        .unwrap();
    app.list_state.select(Some(index));
    app.selected_id = Some(second_id);

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();

    assert_eq!(
        app.layout.preview_content.y,
        app.layout.preview_tabs.y + 2,
        "a tagless preview has metadata, a rule, then content—without a blank tags row"
    );
}

#[test]
fn arrows_sort_and_mouse_use_the_rendered_layout() {
    let (_temporary, library, first_id, second_id) = fixture();
    create_snippet(
        &library,
        &CreateOptions {
            title: "Aardvark".to_owned(),
            folder: Some("Code/Shell".to_owned()),
            language: "text".to_owned(),
            content: String::new(),
            ..CreateOptions::default()
        },
    )
    .unwrap();
    add_fragment(
        &library,
        &first_id.to_string(),
        &FragmentAddOptions {
            title: "helper.sh".to_owned(),
            language: "bash".to_owned(),
            content: "echo helper\n".to_owned(),
            ..FragmentAddOptions::default()
        },
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();

    app.sort = SortMode::Title;
    app.refresh_visible();
    assert_eq!(app.visible[0].snippet_id, first_id, "pinned remains first");
    let second_title = app
        .catalog
        .snippets
        .iter()
        .find(|snippet| snippet.id == app.visible[1].snippet_id)
        .map(|snippet| snippet.title.as_str());
    assert_eq!(second_title, Some("Aardvark"));
    app.handle_key(key(KeyCode::Char('s')));
    assert_eq!(app.sort, SortMode::Modified);

    app.handle_key(key(KeyCode::Right));
    assert_eq!(app.focus, Pane::List);
    app.handle_key(key(KeyCode::Right));
    assert_eq!(app.focus, Pane::Preview);
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.focus, Pane::List);
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.focus, Pane::Sidebar);

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();

    let _ = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 8, 5));
    assert_eq!(app.focus, Pane::Sidebar);
    assert_eq!(app.filter.folder.as_deref(), Some("Code"));

    let _ = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 30, 4));
    assert_eq!(app.focus, Pane::List);
    assert!(app.selected_id.is_some());
    let _ = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 30, 4));
    assert_eq!(app.focus, Pane::Preview, "second click drills into preview");

    app.selected_id = Some(first_id);
    app.list_state.select(
        app.visible
            .iter()
            .position(|row| row.snippet_id == first_id),
    );
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert_eq!(app.layout.tab_count, 2);
    let tab = app.layout.tab_spans[1];
    let _ = app.handle_mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        tab.0,
        app.layout.preview_tabs.y,
    ));
    assert_eq!(app.fragment_index, 1);
    let _ = app.handle_mouse(mouse(
        MouseEventKind::ScrollDown,
        app.layout.preview_content.x,
        app.layout.preview_content.y,
    ));
    assert_eq!(app.preview_scroll, 3);
    assert!(
        app.catalog
            .snippets
            .iter()
            .any(|snippet| snippet.id == second_id)
    );
}

#[test]
fn preview_drag_selection_copies_text_without_line_number_gutter() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();

    // The fixture renders note title, note, footer, then `1│ fn alpha() {}`.
    let x = app.layout.preview_content.x;
    let y = app.layout.preview_content.y + 3;
    let _ = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), x + 3, y));
    assert!(
        app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), x + 3, y,))
            .is_empty(),
        "a plain click must not copy a single character"
    );
    let _ = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), x, y));
    let _ = app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), x + 10, y));
    let effects = app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), x + 10, y));
    let Effect::CopyToClipboard { text, label } = &effects[0] else {
        panic!("expected automatic clipboard effect");
    };
    assert_eq!(text, "fn alpha");
    assert_eq!(label, "selection");
    assert!(!text.contains('1'));
    assert!(!text.contains('│'));

    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert_ne!(buffer.cell((x, y)).unwrap().bg, app.theme.selection_bg);
    assert_eq!(buffer.cell((x + 3, y)).unwrap().bg, app.theme.selection_bg);
}

#[test]
fn help_overlay_accepts_mouse_wheel_scrolling() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.show_help = true;

    let _ = app.handle_mouse(mouse(MouseEventKind::ScrollDown, 40, 12));
    assert_eq!(app.help_scroll, 3);
    let _ = app.handle_mouse(mouse(MouseEventKind::ScrollUp, 40, 12));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn wrapped_code_rows_keep_a_blank_line_number_gutter() {
    let (_temporary, library, first_id, _second_id) = fixture();
    let catalog = library.scan().unwrap();
    let snippet = library
        .resolve_snippet(&catalog, &first_id.to_string())
        .unwrap();
    edit_snippet(
        &library,
        &first_id.to_string(),
        &EditOptions {
            content: Some(format!("value=\"{}\"\necho done\n", "a".repeat(70))),
            if_hash: Some(snippet.fingerprint.clone()),
            ..EditOptions::default()
        },
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let x = app.layout.preview_content.x;
    let first = app.layout.preview_content.y + 3;
    assert_eq!(buffer.cell((x, first)).unwrap().symbol(), "1");
    assert_eq!(buffer.cell((x + 1, first)).unwrap().symbol(), "│");
    for continuation in [first + 1, first + 2] {
        assert_eq!(buffer.cell((x, continuation)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((x + 1, continuation)).unwrap().symbol(), "│");
    }
    assert_eq!(buffer.cell((x, first + 3)).unwrap().symbol(), "2");
}

#[test]
fn snippet_metadata_mutations_flow_through_modals() {
    let (_temporary, library, first_id, _second_id) = fixture();
    let mut app = App::new(library.clone(), &AppConfig::default()).unwrap();
    app.focus = Pane::List;

    app.handle_key(key(KeyCode::Char('r')));
    replace_modal_input(&mut app, "Renamed Rust");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.selected_snippet().unwrap().title, "Renamed Rust");

    app.handle_key(key(KeyCode::Char('t')));
    replace_modal_input(&mut app, "dev, cli");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.selected_snippet().unwrap().tags, ["dev", "cli"]);

    app.handle_key(key(KeyCode::Char('m')));
    let Some(Modal::Picker(picker)) = app.modal.as_mut() else {
        panic!("expected folder picker");
    };
    picker.selected = picker
        .filtered()
        .iter()
        .position(|folder| folder.value == "Code/Shell")
        .unwrap();
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.selected_snippet().unwrap().folder, "Code/Shell");

    let pinned = app.selected_snippet().unwrap().pinned;
    app.handle_key(key(KeyCode::Char('P')));
    assert_eq!(app.selected_snippet().unwrap().pinned, !pinned);
    app.handle_key(key(KeyCode::Char('L')));
    assert!(app.selected_snippet().unwrap().locked);
    app.handle_key(key(KeyCode::Char('r')));
    assert!(app.modal.is_none());
    assert!(app.status.as_ref().unwrap().text.contains("locked"));
    app.handle_key(key(KeyCode::Char('L')));
    assert!(!app.selected_snippet().unwrap().locked);

    app.handle_key(key(KeyCode::Char('d')));
    assert!(matches!(app.modal, Some(Modal::Confirm(_))));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(
        !app.catalog
            .snippets
            .iter()
            .any(|snippet| snippet.id == first_id)
    );
    assert_eq!(snip::service::trash_entries(&library).unwrap().len(), 1);
}

#[test]
fn git_panel_key_routing_badge_and_missing_binary_gate_work() {
    let temporary = tempfile::tempdir().unwrap();
    let library = Library::init(
        &temporary.path().join("Git panel.sniplib"),
        Some("Git panel"),
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    assert!(matches!(
        app.git.unavailable,
        Some(Unavailable::NotARepository)
    ));

    let ctrl_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
    app.show_help = true;
    app.handle_key(ctrl_g);
    assert!(app.git.open);
    assert!(!app.show_help);
    app.handle_key(key(KeyCode::Esc));

    app.trash.open = true;
    app.handle_key(ctrl_g);
    assert!(app.git.open);
    assert!(!app.trash.open);

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
    assert!(rendered.contains("This library is not a git repository."));
    assert!(rendered.contains("git init"));
    assert!(rendered.contains("initialize"));
    assert!(matches!(
        app.handle_key(key(KeyCode::Char('i'))).as_slice(),
        [Effect::RunGit(GitAction::Init)]
    ));
    snip::git::init(app.library.root()).unwrap();
    app.reprobe_git();
    assert!(app.git.repo.is_some());
    assert!(app.git.unavailable.is_none());

    app.handle_key(key(KeyCode::Char('r')));
    assert!(app.git.open, "refresh is swallowed by the Git panel");
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.git.open);

    app.git.unavailable = None;
    app.git.status = Some(Status {
        branch: Branch::Named {
            name: "main".to_owned(),
        },
        upstream: Some("origin/main".to_owned()),
        ahead: 1,
        behind: 0,
        staged: 0,
        unstaged: 2,
        untracked: 0,
        conflicted: Vec::new(),
        state: RepoState::Clean,
        head_oid: Some("abcdef123".to_owned()),
        last_commit: None,
        upstream_commit: None,
    });
    app.git.auto_commit_interval = 15;
    app.git.auto_push = true;
    app.git.backup_on_quit = true;
    app.git.last_commit_error = Some("previous commit failure".to_owned());
    app.git.last_push_error = Some("previous push failure".to_owned());
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    let top = row_text(terminal.backend().buffer(), 0);
    assert!(top.contains("git:main +2 ^1"));

    app.git.open = true;
    app.handle_key(key(KeyCode::Char('a')));
    assert!(app.git.auto_backup_paused);
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert!(row_text(terminal.backend().buffer(), 0).contains("[auto paused]"));
    app.handle_key(key(KeyCode::Char('a')));
    assert!(!app.git.auto_backup_paused);
    assert!(matches!(
        app.handle_key(key(KeyCode::Char('b'))).as_slice(),
        [Effect::RunGit(GitAction::Backup)]
    ));
    app.git.status.as_mut().unwrap().unstaged = 0;
    assert!(matches!(
        app.handle_key(key(KeyCode::Char('b'))).as_slice(),
        [Effect::RunGit(GitAction::Backup)]
    ));
    app.git.status.as_mut().unwrap().ahead = 0;
    assert!(matches!(
        app.handle_key(key(KeyCode::Char('b'))).as_slice(),
        [Effect::RunGit(GitAction::Backup)]
    ));
    app.git.status.as_mut().unwrap().ahead = 1;
    app.git.status.as_mut().unwrap().unstaged = 2;
    assert!(matches!(
        app.handle_key(key(KeyCode::Char('c'))).as_slice(),
        [Effect::RunGit(GitAction::Commit { message: None })]
    ));
    assert!(matches!(
        app.handle_key(key(KeyCode::Char('p'))).as_slice(),
        [Effect::RunGit(GitAction::Push)]
    ));
    app.handle_key(key(KeyCode::Char('C')));
    let Some(Modal::Input(message)) = app.modal.as_ref() else {
        panic!("custom Git message should open an input modal");
    };
    assert!(message.value.starts_with("snip backup:"));
    replace_modal_input(&mut app, "custom backup");
    assert!(matches!(
        app.handle_key(key(KeyCode::Enter)).as_slice(),
        [Effect::RunGit(GitAction::Commit {
            message: Some(message)
        })] if message == "custom backup"
    ));

    app.git.push_in_flight = true;
    app.git.push_attempted_at = Some(Instant::now() - Duration::from_secs(181));
    terminal
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert!(row_text(terminal.backend().buffer(), 0).contains(">>"));
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("WORKTREE"));
    assert!(rendered.contains("AUTOMATION"));
    assert!(rendered.contains("commit every 15 min"));
    assert!(rendered.contains("push after commit"));
    assert!(rendered.contains("background push stalled"));
    assert!(rendered.contains("previous commit failure"));
    assert!(rendered.contains("previous push failure"));
    assert!(rendered.contains("origin/main"));
    assert!(rendered.contains("backup"));
    assert!(rendered.contains("message"));

    for width in [58, 50, 40] {
        let backend = TestBackend::new(width, 32);
        let mut narrow_console = Terminal::new(backend).unwrap();
        narrow_console
            .draw(|frame| snip::tui::ui::draw(frame, &mut app))
            .unwrap();
        let rendered = narrow_console
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for action in ["backup", "fetch", "on quit", "message", "close"] {
            assert!(
                rendered.contains(action),
                "{action} footer action was clipped at {width} columns"
            );
        }
    }
    app.git.open = false;

    let narrow_backend = TestBackend::new(59, 24);
    let mut narrow = Terminal::new(narrow_backend).unwrap();
    narrow
        .draw(|frame| snip::tui::ui::draw(frame, &mut app))
        .unwrap();
    assert!(!row_text(narrow.backend().buffer(), 0).contains("git:main"));

    let status = app.git.status.as_mut().unwrap();
    status.state = RepoState::Merging;
    status.conflicted = vec!["one/snippet.toml".to_owned(), "two/snippet.toml".to_owned()];
    app.git.open = true;
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
    assert!(rendered.contains("conflicts in 2 files"));
    assert!(
        rendered.contains("refresh"),
        "dynamic height must keep the footer visible in attention states"
    );
    app.git.open = false;

    app.git.status = None;
    app.git.unavailable = Some(Unavailable::ProbeFailed {
        message: "cannot execute git".to_owned(),
    });
    app.handle_key(ctrl_g);
    assert!(app.git.open);
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
    assert!(rendered.contains("Git could not inspect this library."));
    assert!(rendered.contains("cannot execute git"));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    app.git.status = None;
    app.git.unavailable = Some(Unavailable::BinaryMissing);
    app.handle_key(ctrl_g);
    assert!(!app.git.open);
    assert_eq!(
        app.status.as_ref().map(|status| status.text.as_str()),
        Some("git not found in PATH")
    );
}

#[test]
fn quit_backup_waits_for_its_git_effect_to_finish() {
    let temporary = tempfile::tempdir().unwrap();
    let library = Library::init(
        &temporary.path().join("Quit backup.sniplib"),
        Some("Quit backup"),
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.git.backup_on_quit = true;
    app.git.unavailable = None;
    app.git.status = Some(Status {
        branch: Branch::Named {
            name: "main".to_owned(),
        },
        upstream: None,
        ahead: 0,
        behind: 0,
        staged: 0,
        unstaged: 1,
        untracked: 0,
        conflicted: Vec::new(),
        state: RepoState::Clean,
        head_oid: Some("abcdef123".to_owned()),
        last_commit: None,
        upstream_commit: None,
    });

    assert!(matches!(
        app.handle_key(key(KeyCode::Char('q'))).as_slice(),
        [Effect::RunGit(GitAction::Backup)]
    ));
    assert!(app.pending_quit);
    assert!(!app.should_quit);
    assert!(app.git.operation_queued);

    app.finish_git_operation();
    assert!(app.should_quit);
    assert!(!app.git.operation_queued);
}

#[test]
fn quit_backup_runs_for_a_clean_worktree_with_unpushed_commits() {
    let temporary = tempfile::tempdir().unwrap();
    let library = Library::init(
        &temporary.path().join("Clean ahead.sniplib"),
        Some("Clean ahead"),
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.git.backup_on_quit = true;
    app.git.unavailable = None;
    app.git.status = Some(Status {
        branch: Branch::Named {
            name: "main".to_owned(),
        },
        upstream: Some("origin/main".to_owned()),
        ahead: 4,
        behind: 0,
        staged: 0,
        unstaged: 0,
        untracked: 0,
        conflicted: Vec::new(),
        state: RepoState::Clean,
        head_oid: Some("abcdef123".to_owned()),
        last_commit: None,
        upstream_commit: None,
    });

    assert!(matches!(
        app.handle_key(key(KeyCode::Char('q'))).as_slice(),
        [Effect::RunGit(GitAction::Backup)]
    ));
    assert!(app.pending_quit);
    assert!(!app.should_quit);
}

#[test]
fn auto_commit_honors_interlocks_and_lock_contention() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Automatic.sniplib");
    let library = Library::init(&root, Some("Automatic")).unwrap();
    init_git_repo(&root);
    let repo = snip::git::probe(library.root()).unwrap();
    snip::git::commit(&repo, "initial").unwrap();
    let config = AppConfig {
        git: Some(GitConfig {
            auto_commit_interval: 1,
            backup_on_quit: false,
            ..GitConfig::default()
        }),
        ..AppConfig::default()
    };
    let mut app = App::new(library.clone(), &config).unwrap();

    let tags_path = root.join("tags.toml");
    let mut tags = std::fs::read_to_string(&tags_path).unwrap();
    tags.push_str("\n# automatic\n");
    std::fs::write(&tags_path, tags).unwrap();
    app.refresh_git();
    app.git
        .status
        .as_mut()
        .unwrap()
        .last_commit
        .as_mut()
        .unwrap()
        .timestamp -= 120;

    app.git.operation_queued = true;
    app.tick_auto_backup();
    assert!(snip::git::status(&repo).unwrap().dirty_count() > 0);
    app.git.operation_queued = false;

    app.modal = Some(Modal::Input(InputModal::new(
        "Editing",
        "",
        ModalAction::GitCommit,
    )));
    app.tick_auto_backup();
    assert!(snip::git::status(&repo).unwrap().dirty_count() > 0);
    app.modal = None;

    let library_lock = library.lock().unwrap();
    app.tick_auto_backup();
    assert!(app.git.last_commit_error.is_none());
    assert!(app.status.is_none());
    drop(library_lock);

    app.git.last_push_error = Some("previous push failure".to_owned());
    app.git.auto_attempted_at = None;
    app.tick_auto_backup();
    assert_eq!(snip::git::status(&repo).unwrap().dirty_count(), 0);
    assert!(app.git.last_commit_error.is_none());
    assert_eq!(
        app.git.last_push_error.as_deref(),
        Some("previous push failure")
    );
}

#[test]
fn auto_push_requires_an_opted_in_sender_and_blocks_manual_git_while_running() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let library = Library::init(
        &temporary.path().join("Push guards.sniplib"),
        Some("Push guards"),
    )
    .unwrap();
    init_git_repo(library.root());
    let repo = snip::git::probe(library.root()).unwrap();
    snip::git::commit(&repo, "initial").unwrap();
    let config = AppConfig {
        git: Some(GitConfig {
            auto_commit_interval: 1,
            auto_push: true,
            ..GitConfig::default()
        }),
        ..AppConfig::default()
    };
    let mut app = App::new(library, &config).unwrap();
    app.git.status = Some(Status {
        branch: Branch::Named {
            name: "main".to_owned(),
        },
        upstream: Some("origin/main".to_owned()),
        ahead: 1,
        behind: 0,
        staged: 0,
        unstaged: 0,
        untracked: 0,
        conflicted: Vec::new(),
        state: RepoState::Clean,
        head_oid: Some("abcdef123".to_owned()),
        last_commit: None,
        upstream_commit: None,
    });

    app.tick_auto_backup();
    assert!(!app.git.push_in_flight);
    assert!(app.git.push_attempted_at.is_none());

    let (sender, receiver) = mpsc::channel();
    app.set_git_sender(sender);
    app.git.auto_push = false;
    app.tick_auto_backup();
    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
    assert!(app.git.push_attempted_at.is_none());

    app.git.auto_push = true;
    app.git.push_in_flight = true;
    app.tick_auto_backup();
    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());

    app.git.open = true;
    assert!(app.handle_key(key(KeyCode::Char('p'))).is_empty());
    assert!(app.handle_key(key(KeyCode::Char('C'))).is_empty());
    assert!(app.modal.is_none());
    assert_eq!(
        app.status.as_ref().map(|status| status.text.as_str()),
        Some("a background push is running")
    );

    app.git.open = false;
    app.status = None;
    assert!(app.handle_key(key(KeyCode::Char('q'))).is_empty());
    assert!(app.pending_quit);
    assert!(!app.should_quit);
    assert!(!app.git.operation_queued);
    assert_eq!(
        app.status.as_ref().map(|status| status.text.as_str()),
        Some("finishing background push…")
    );
    app.git.push_in_flight = false;
    assert!(app.handle_key(key(KeyCode::Char('q'))).is_empty());
    assert!(app.should_quit);
}

#[test]
fn background_push_reports_success_and_advances_a_bare_remote() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Background push.sniplib");
    let bare = temporary.path().join("origin.git");
    let library = Library::init(&root, Some("Background push")).unwrap();
    init_git_repo(&root);
    let repo = snip::git::probe(&root).unwrap();
    snip::git::commit(&repo, "initial").unwrap();
    git_ok(
        temporary.path(),
        &["init", "--bare", bare.to_str().unwrap()],
    );
    git_ok(&root, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(&root, &["push", "-u", "origin", "main"]);
    let mut tags = std::fs::read_to_string(root.join("tags.toml")).unwrap();
    tags.push_str("\n# ahead\n");
    std::fs::write(root.join("tags.toml"), tags).unwrap();
    snip::git::commit(&repo, "background push target").unwrap();

    let config = AppConfig {
        git: Some(GitConfig {
            auto_commit_interval: 1,
            auto_push: true,
            ..GitConfig::default()
        }),
        ..AppConfig::default()
    };
    let mut app = App::new(library, &config).unwrap();
    let (sender, receiver) = mpsc::channel();
    app.set_git_sender(sender);
    app.tick_auto_backup();
    assert!(app.git.push_in_flight);

    let event = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    let AppEvent::GitFinished(result) = event else {
        panic!("expected background Git result");
    };
    assert!(matches!(
        result.outcome,
        Ok(ref outcome) if outcome.pushed && !outcome.committed
    ));
    app.git.last_commit_error = Some("automatic commit is still failing".to_owned());
    app.status = None;
    app.handle_git_task(result);
    assert!(!app.git.push_in_flight);
    assert_eq!(app.git.status.as_ref().unwrap().ahead, 0);
    assert!(app.status.is_none());
    assert!(app.git.last_push_error.is_none());
    assert_eq!(
        app.git.last_commit_error.as_deref(),
        Some("automatic commit is still failing")
    );

    let remote_head = ProcessCommand::new("git")
        .args([
            "--git-dir",
            bare.to_str().unwrap(),
            "log",
            "-1",
            "--format=%s",
        ])
        .output()
        .unwrap();
    assert!(remote_head.status.success());
    assert_eq!(
        String::from_utf8_lossy(&remote_head.stdout).trim(),
        "background push target"
    );
}

#[test]
fn repeated_background_push_failures_emit_only_one_error_transition() {
    if !git_available() {
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("Failed background push.sniplib");
    let bare = temporary.path().join("origin.git");
    let missing = temporary.path().join("missing.git");
    let library = Library::init(&root, Some("Failed background push")).unwrap();
    init_git_repo(&root);
    let repo = snip::git::probe(&root).unwrap();
    snip::git::commit(&repo, "initial").unwrap();
    git_ok(
        temporary.path(),
        &["init", "--bare", bare.to_str().unwrap()],
    );
    git_ok(&root, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git_ok(&root, &["push", "-u", "origin", "main"]);
    git_ok(
        &root,
        &["remote", "set-url", "origin", missing.to_str().unwrap()],
    );
    let mut tags = std::fs::read_to_string(root.join("tags.toml")).unwrap();
    tags.push_str("\n# cannot push\n");
    std::fs::write(root.join("tags.toml"), tags).unwrap();
    snip::git::commit(&repo, "unpushable").unwrap();

    let config = AppConfig {
        git: Some(GitConfig {
            auto_commit_interval: 1,
            auto_push: true,
            ..GitConfig::default()
        }),
        ..AppConfig::default()
    };
    let mut app = App::new(library, &config).unwrap();
    let (sender, receiver) = mpsc::channel();
    app.set_git_sender(sender);

    app.tick_auto_backup();
    let AppEvent::GitFinished(first) = receiver.recv_timeout(Duration::from_secs(5)).unwrap()
    else {
        panic!("expected failed push result");
    };
    assert!(first.outcome.is_err());
    app.handle_git_task(first);
    assert!(
        app.status
            .as_ref()
            .is_some_and(|status| status.text.contains("background push failed"))
    );
    assert!(app.git.last_push_error.is_some());

    app.status = None;
    app.git.push_attempted_at = None;
    app.tick_auto_backup();
    let AppEvent::GitFinished(second) = receiver.recv_timeout(Duration::from_secs(5)).unwrap()
    else {
        panic!("expected repeated failed push result");
    };
    app.handle_git_task(second);
    assert!(app.status.is_none());
    assert!(!app.git.push_in_flight);
}

#[test]
fn sidebar_folder_and_tag_management_reports_service_errors_in_modal() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();

    select_sidebar_item(&mut app, SidebarItem::All);
    app.handle_key(key(KeyCode::Char('n')));
    replace_modal_input(&mut app, "Empty");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.catalog.folders.contains(&"Empty".to_owned()));

    select_sidebar_item(&mut app, SidebarItem::Folder("Empty".to_owned()));
    app.handle_key(key(KeyCode::Char('r')));
    replace_modal_input(&mut app, "Renamed Empty");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.catalog.folders.contains(&"Renamed Empty".to_owned()));

    select_sidebar_item(&mut app, SidebarItem::Folder("Renamed Empty".to_owned()));
    app.handle_key(key(KeyCode::Char('d')));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(!app.catalog.folders.contains(&"Renamed Empty".to_owned()));

    select_sidebar_item(&mut app, SidebarItem::Folder("Code/Rust".to_owned()));
    app.handle_key(key(KeyCode::Char('d')));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(matches!(
        app.modal,
        Some(Modal::Confirm(ref modal)) if modal.error.as_deref().is_some_and(|error| error.contains("not empty"))
    ));
    app.handle_key(key(KeyCode::Esc));

    select_sidebar_item(&mut app, SidebarItem::Tag("dev".to_owned()));
    app.handle_key(key(KeyCode::Char('r')));
    replace_modal_input(&mut app, "craft");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.modal.is_none(), "rename failed: {:?}", app.modal);
    assert!(
        app.catalog.tags.contains(&"craft".to_owned()),
        "catalog tags: {:?}",
        app.catalog.tags
    );
    select_sidebar_item(&mut app, SidebarItem::Tag("craft".to_owned()));
    app.handle_key(key(KeyCode::Char('d')));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(!app.catalog.tags.contains(&"craft".to_owned()));
}

#[test]
fn create_wizard_uses_defaults_and_opens_the_new_fragment_editor() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let config = AppConfig {
        default_language: Some("python".to_owned()),
        default_folder: Some("Code/Rust".to_owned()),
        default_tags: vec!["generated".to_owned()],
        ..AppConfig::default()
    };
    let mut app = App::new(library, &config).unwrap();
    app.focus = Pane::List;

    app.handle_key(key(KeyCode::Char('n')));
    replace_modal_input(&mut app, "Generated helper");
    app.handle_key(key(KeyCode::Enter));
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected folder picker");
    };
    assert_eq!(picker.selected_value().as_deref(), Some("Code/Rust"));
    app.handle_key(key(KeyCode::Enter));
    let Some(Modal::Picker(language)) = app.modal.as_ref() else {
        panic!("expected language picker");
    };
    assert_eq!(language.selected_value().as_deref(), Some("python"));
    assert!(language.allow_custom);
    let effects = app.handle_key(key(KeyCode::Enter));
    let Effect::SpawnEditor(request) = effects.into_iter().next().unwrap() else {
        panic!("expected editor for newly created snippet");
    };
    assert!(matches!(request.target, EditTarget::Content { .. }));
    assert_eq!(request.original, "");
    assert_eq!(request.suffix, "py");
    let created = app.selected_snippet().unwrap();
    assert_eq!(created.title, "Generated helper");
    assert_eq!(created.folder, "Code/Rust");
    assert_eq!(created.tags, ["generated"]);
}

#[test]
fn create_language_picker_accepts_aliases_and_custom_values() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;

    app.handle_key(key(KeyCode::Char('n')));
    replace_modal_input(&mut app, "Typed helper");
    app.handle_key(key(KeyCode::Enter));
    app.handle_key(key(KeyCode::Enter));
    for character in "ts".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected language picker");
    };
    assert_eq!(picker.selected_value().as_deref(), Some("typescript"));

    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Backspace));
    for character in "my-dsl".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected language picker");
    };
    assert_eq!(picker.selected, 0);
    assert_eq!(picker.selected_value().as_deref(), Some("my-dsl"));
}

#[test]
fn edit_language_keeps_manifest_file_badge_and_preview_in_sync() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;
    let old_path = app.selected_snippet().unwrap().loaded_fragments[0]
        .absolute_path
        .clone();

    app.handle_key(key(KeyCode::Char('f')));
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected language picker");
    };
    assert_eq!(picker.current_value.as_deref(), Some("rust"));
    assert_eq!(picker.selected_value().as_deref(), Some("rust"));
    for character in "python".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    app.handle_key(key(KeyCode::Enter));

    let snippet = app.selected_snippet().unwrap();
    let fragment = &snippet.loaded_fragments[0];
    assert_eq!(fragment.language, "python");
    assert_eq!(fragment.file, "fragments/001-Alpha Rust.py");
    assert!(fragment.absolute_path.exists());
    assert!(!old_path.exists());
    assert_eq!(snip::tui::icons::language_badge(&fragment.language), "py");

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
    assert!(rendered.contains("001-Alpha Rust.py py"));
}

#[test]
fn edit_language_only_changes_the_current_fragment() {
    let (_temporary, library, first_id, _second_id) = fixture();
    add_fragment(
        &library,
        &first_id.to_string(),
        &FragmentAddOptions {
            title: "Helper".to_owned(),
            language: "bash".to_owned(),
            content: "echo helper\n".to_owned(),
            ..FragmentAddOptions::default()
        },
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::Preview;
    app.fragment_index = 1;

    app.handle_key(key(KeyCode::Char('f')));
    for character in "typescript".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    app.handle_key(key(KeyCode::Enter));

    let snippet = app.selected_snippet().unwrap();
    assert_eq!(snippet.loaded_fragments[0].language, "rust");
    assert_eq!(
        snippet.loaded_fragments[0].file,
        "fragments/001-Alpha Rust.rs"
    );
    assert_eq!(snippet.loaded_fragments[1].language, "typescript");
    assert_eq!(snippet.loaded_fragments[1].file, "fragments/002-Helper.ts");
    assert_eq!(snip::tui::icons::snippet_badge(snippet), "mix");
}

#[test]
fn edit_language_refuses_locked_snippets() {
    let (_temporary, library, first_id, _second_id) = fixture();
    edit_snippet(
        &library,
        &first_id.to_string(),
        &EditOptions {
            locked: Some(true),
            ..EditOptions::default()
        },
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;

    app.handle_key(key(KeyCode::Char('f')));

    assert!(app.modal.is_none());
    assert!(app.status.as_ref().unwrap().text.contains("locked"));
    assert_eq!(
        app.selected_snippet().unwrap().loaded_fragments[0].language,
        "rust"
    );
}

#[test]
fn edit_language_accepts_a_custom_value_without_an_extension() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;

    app.handle_key(key(KeyCode::Char('f')));
    for character in "my-dsl".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    app.handle_key(key(KeyCode::Enter));

    let fragment = &app.selected_snippet().unwrap().loaded_fragments[0];
    assert_eq!(fragment.language, "my-dsl");
    assert_eq!(fragment.file, "fragments/001-Alpha Rust");
    assert!(fragment.absolute_path.exists());
    assert_eq!(snip::tui::icons::language_badge(&fragment.language), "?");
}

#[test]
fn list_and_preview_share_the_same_bottom_bar_actions() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    let render = |app: &mut App| {
        let backend = TestBackend::new(140, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| snip::tui::ui::draw(frame, app))
            .unwrap();
        row_text(terminal.backend().buffer(), 23)
    };

    app.focus = Pane::List;
    let list = render(&mut app);
    app.focus = Pane::Preview;
    let preview = render(&mut app);

    assert_eq!(list, preview);
    for action in ["create", "edit", "tags", "rename", "move", "copy", "path"] {
        assert!(
            list.contains(action),
            "missing {action} from full bottom bar"
        );
    }
    assert!(!list.contains("language"));
    assert!(!list.contains("vscode"));
}

#[test]
fn tui_config_controls_theme_sort_and_density() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let config = AppConfig {
        tui: Some(TuiConfig {
            theme: TuiThemeSetting::Light,
            sort: SortMode::Title,
            density: TuiDensitySetting::Compact,
            ..TuiConfig::default()
        }),
        ..AppConfig::default()
    };
    let app = App::new(library, &config).unwrap();
    assert_eq!(app.theme.appearance, Appearance::Light);
    assert_eq!(app.sort, SortMode::Title);
    assert_eq!(app.density, TuiDensitySetting::Compact);
    assert_eq!(app.icon_mode, IconMode::Ascii);
}

#[test]
fn compact_density_keeps_pinned_state_visible() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let config = AppConfig {
        tui: Some(TuiConfig {
            density: TuiDensitySetting::Compact,
            ..TuiConfig::default()
        }),
        ..AppConfig::default()
    };
    let mut app = App::new(library, &config).unwrap();
    let backend = TestBackend::new(80, 24);
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
    assert!(
        rendered.contains("★"),
        "compact rows must expose pinned state"
    );
}

#[test]
fn note_and_readme_editor_targets_save_markdown() {
    let (_temporary, library, first_id, _second_id) = fixture();
    let mut app = App::new(library.clone(), &AppConfig::default()).unwrap();
    app.focus = Pane::List;
    app.selected_id = Some(first_id);
    app.list_state.select(Some(
        app.visible
            .iter()
            .position(|row| row.snippet_id == first_id)
            .unwrap(),
    ));

    let effects = app.handle_key(key(KeyCode::Char('E')));
    let Effect::SpawnEditor(mut note) = effects.into_iter().next().unwrap() else {
        panic!("expected note editor");
    };
    assert!(matches!(note.target, EditTarget::Note { .. }));
    assert_eq!(note.suffix, "md");
    note.edited = Some("updated **note**\n".to_owned());
    force_save(&library, &note).unwrap();
    app.rescan().unwrap();
    assert_eq!(
        app.selected_snippet().unwrap().loaded_fragments[0]
            .note_content
            .as_deref(),
        Some("updated **note**\n")
    );

    let effects = app.handle_key(key(KeyCode::Char('R')));
    let Effect::SpawnEditor(mut readme) = effects.into_iter().next().unwrap() else {
        panic!("expected readme editor");
    };
    assert_eq!(readme.target, EditTarget::Readme);
    assert_eq!(readme.suffix, "md");
    readme.edited = Some("# README\n".to_owned());
    force_save(&library, &readme).unwrap();
    app.rescan().unwrap();
    assert_eq!(
        app.selected_snippet().unwrap().readme.as_deref(),
        Some("# README\n")
    );
}
#[test]
fn v_key_emits_open_in_vscode_effect() {
    let (_temporary, library, first_id, _second_id) = fixture();
    let config = AppConfig {
        vscode_cmd: Some("code-insiders".to_owned()),
        ..AppConfig::default()
    };
    let mut app = App::new(library, &config).unwrap();
    app.focus = Pane::List;
    app.selected_id = Some(first_id);
    app.list_state.select(Some(
        app.visible
            .iter()
            .position(|row| row.snippet_id == first_id)
            .unwrap(),
    ));

    assert_eq!(app.vscode_cmd.as_deref(), Some("code-insiders"));

    let effects = app.handle_key(key(KeyCode::Char('v')));
    let Effect::OpenInVsCode { path } = effects.into_iter().next().unwrap() else {
        panic!("expected OpenInVsCode effect");
    };

    let snippet = app.selected_snippet().unwrap();
    let expected_path = &snippet.loaded_fragments[0].absolute_path;
    assert_eq!(&path, expected_path);
}

#[test]
fn trash_overlay_restores_and_purges_entries() {
    let (_temporary, library, first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;

    app.handle_key(key(KeyCode::Char('d')));
    app.handle_key(key(KeyCode::Char('y')));
    app.handle_key(key(KeyCode::Char('T')));
    assert!(app.trash.open);
    assert_eq!(app.trash.entries.len(), 1);
    app.handle_key(key(KeyCode::Char('u')));
    assert!(
        app.catalog
            .snippets
            .iter()
            .any(|snippet| snippet.id == first_id)
    );
    assert!(app.trash.entries.is_empty());

    app.handle_key(key(KeyCode::Esc));
    app.selected_id = Some(first_id);
    app.list_state.select(
        app.visible
            .iter()
            .position(|row| row.snippet_id == first_id),
    );
    app.handle_key(key(KeyCode::Char('d')));
    app.handle_key(key(KeyCode::Char('y')));
    app.handle_key(key(KeyCode::Char('T')));
    app.handle_key(key(KeyCode::Char('x')));
    assert!(matches!(app.modal, Some(Modal::Confirm(_))));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(app.trash.entries.is_empty());
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
        target: EditTarget::Content {
            fragment_id: fragment.id,
        },
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

#[test]
fn copy_snippet_path_effect_copies_absolute_path() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();

    let expected_path = app.selected_snippet().unwrap().loaded_fragments[0]
        .absolute_path
        .display()
        .to_string();

    let effects = app.handle_key(key(KeyCode::Char('p')));
    let Effect::CopyToClipboard { text, label } = &effects[0] else {
        panic!("expected copy to clipboard effect");
    };
    assert_eq!(text, &expected_path);
    assert_eq!(label, "snippet path");
}

#[test]
fn sidebar_uncategorized_and_trash_items_work() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    create_snippet(
        &library,
        &CreateOptions {
            title: "Root Snippet".to_owned(),
            folder: None,
            language: "text".to_owned(),
            content: "root content\n".to_owned(),
            ..CreateOptions::default()
        },
    )
    .unwrap();

    let mut app = App::new(library, &AppConfig::default()).unwrap();
    let uncat_row = app
        .sidebar
        .rows
        .iter()
        .position(|row| row.item == SidebarItem::Uncategorized)
        .unwrap();
    app.sidebar.list_state.select(Some(uncat_row));
    app.focus = Pane::Sidebar;
    app.handle_key(key(KeyCode::Enter));

    assert_eq!(app.visible.len(), 1);
    assert_eq!(app.selected_snippet().unwrap().title, "Root Snippet");

    let trash_row = app
        .sidebar
        .rows
        .iter()
        .position(|row| row.item == SidebarItem::Trash)
        .unwrap();
    app.sidebar.list_state.select(Some(trash_row));
    app.focus = Pane::Sidebar;
    app.handle_key(key(KeyCode::Enter));
    assert!(app.trash.open);
}

#[test]
fn picker_filter_accepts_j_and_k_and_navigates_with_arrows() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;
    snip::service::create_folder(&app.library, "Docker").unwrap();
    app.rescan().unwrap();

    app.handle_key(key(KeyCode::Char('m')));
    for character in "Docker".chars() {
        app.handle_key(key(KeyCode::Char(character)));
    }
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected folder picker");
    };
    // `j`/`k` must reach the filter instead of moving the selection.
    assert_eq!(picker.filter, "Docker");
    assert_eq!(picker.selected_value().as_deref(), Some("Docker"));

    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Backspace));
    app.handle_key(key(KeyCode::Down));
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected folder picker");
    };
    let after_down = picker.selected;
    assert_eq!(after_down, 1);
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected folder picker");
    };
    assert_eq!(picker.selected, 2);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Up));
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected folder picker");
    };
    assert_eq!(picker.selected, 0);
}

#[test]
fn folder_pickers_label_the_library_root_the_way_the_cli_prints_it() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;

    app.handle_key(key(KeyCode::Char('m')));
    let Some(Modal::Picker(picker)) = app.modal.as_ref() else {
        panic!("expected folder picker");
    };
    // The root reads as `Uncategorized` (as in `snip list`) but submits an empty
    // folder path, so a real folder of that name could never shadow it.
    assert_eq!(picker.items[0].label, snip::UNCATEGORIZED);
    assert_eq!(picker.items[0].value, "");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.selected_snippet().unwrap().folder, "");
}

#[test]
fn folder_rename_keeps_the_parent_and_move_reparents() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();

    // `r` mirrors `snip folder rename`: one path component, parent untouched.
    select_sidebar_item(&mut app, SidebarItem::Folder("Code/Rust".to_owned()));
    app.handle_key(key(KeyCode::Char('r')));
    let Some(Modal::Input(input)) = app.modal.as_ref() else {
        panic!("expected rename input");
    };
    assert_eq!(input.value, "Rust", "rename prefills the leaf name only");
    replace_modal_input(&mut app, "Systems/Rust");
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(
        app.modal,
        Some(Modal::Input(ref modal))
            if modal.error.as_deref().is_some_and(|error| error.contains("one path component"))
    ));
    replace_modal_input(&mut app, "Rustic");
    app.handle_key(key(KeyCode::Enter));
    assert!(app.catalog.folders.contains(&"Code/Rustic".to_owned()));

    // `m` mirrors `snip folder move`: the picked folder becomes the new parent.
    select_sidebar_item(&mut app, SidebarItem::Folder("Code/Rustic".to_owned()));
    app.handle_key(key(KeyCode::Char('m')));
    let Some(Modal::Picker(picker)) = app.modal.as_mut() else {
        panic!("expected folder picker");
    };
    assert!(
        !picker.items.iter().any(|item| item.value == "Code/Rustic"),
        "a folder cannot move into itself"
    );
    picker.selected = picker
        .filtered()
        .iter()
        .position(|item| item.value == "Code/Shell")
        .unwrap();
    app.handle_key(key(KeyCode::Enter));
    assert!(
        app.catalog
            .folders
            .contains(&"Code/Shell/Rustic".to_owned()),
        "folders: {:?}",
        app.catalog.folders
    );
}

#[test]
fn both_rescan_bindings_work_independently() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library.clone(), &AppConfig::default()).unwrap();

    // A match guard on `F(5) | Char('r')` would apply to both alternatives and
    // silently demand Ctrl-F5, so each binding is checked on its own.
    app.handle_key(key(KeyCode::F(5)));
    assert_eq!(
        app.status.as_ref().map(|status| status.text.as_str()),
        Some("library refreshed"),
        "plain F5 should rescan"
    );

    app.status = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(
        app.status.as_ref().map(|status| status.text.as_str()),
        Some("library refreshed"),
        "Ctrl-r should rescan"
    );

    // Plain `r` must still fall through to rename rather than rescanning.
    app.status = None;
    app.focus = Pane::List;
    app.handle_key(key(KeyCode::Char('r')));
    assert!(matches!(app.modal, Some(Modal::Input(_))), "r opens rename");
}
#[test]
fn ctrl_d_scrolls_list_instead_of_triggering_delete() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;
    app.list_state.select(Some(0));

    let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_d);

    assert!(
        app.modal.is_none(),
        "Ctrl-d must not trigger delete confirmation modal"
    );
    let expected_selected = app.visible.len().saturating_sub(1);
    assert_eq!(
        app.list_state.selected(),
        Some(expected_selected),
        "Ctrl-d should move selection down (clamped to list end)"
    );

    // Plain `d` must still trigger delete.
    app.handle_key(key(KeyCode::Char('d')));
    assert!(
        matches!(app.modal, Some(Modal::Confirm(_))),
        "plain d must open delete confirmation modal"
    );
}
#[test]
fn paragraph_jump_keys_work_in_preview() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let snippet = create_snippet(
        &library,
        &CreateOptions {
            title: "Multi Paragraph".to_owned(),
            content:
                "para 1 line A\npara 1 line B\n\npara 2 line A\npara 2 line B\n\npara 3 line A"
                    .to_owned(),
            ..CreateOptions::default()
        },
    )
    .unwrap();

    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;
    app.selected_id = Some(snippet.id);
    app.refresh_visible();
    app.focus = Pane::Preview;

    assert_eq!(app.preview_scroll, 0);

    // Jump forward to first blank line
    app.handle_key(key(KeyCode::Char('}')));
    assert!(
        app.preview_scroll > 0,
        "should jump to first paragraph boundary"
    );
    let first_jump = app.preview_scroll;

    // Jump forward again to second blank line
    app.handle_key(key(KeyCode::Char('}')));
    assert!(
        app.preview_scroll > first_jump,
        "should jump to second paragraph boundary"
    );

    // Jump backward
    app.handle_key(key(KeyCode::Char('{')));
    assert_eq!(
        app.preview_scroll, first_jump,
        "should jump back to first paragraph boundary"
    );

    app.handle_key(key(KeyCode::Char('{')));
    assert_eq!(app.preview_scroll, 0, "should jump back to top");
}
#[test]
fn ctrl_modified_keys_do_not_trigger_plain_actions() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    app.focus = Pane::List;

    // Ctrl-u should scroll instead of doing any plain 'u' action
    let ctrl_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_u);
    assert!(app.modal.is_none());

    // Ctrl-n should not open create modal
    let ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_n);
    assert!(app.modal.is_none());

    // Ctrl-p should not toggle pin
    let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
    app.handle_key(ctrl_p);
    assert!(
        app.selected_snippet().unwrap().pinned,
        "Ctrl-p should not toggle pin state from true to false"
    );
}
#[test]
fn digit_keys_jump_to_items_or_fragments_in_panes() {
    let (_temporary, library, first_id, second_id) = fixture();
    let mut app = App::new(library, &AppConfig::default()).unwrap();

    // 1. Sidebar Pane navigation
    app.focus = Pane::Sidebar;
    app.handle_key(key(KeyCode::Char('2')));
    assert_eq!(app.sidebar.list_state.selected(), Some(1));

    app.handle_key(key(KeyCode::Char('1')));
    assert_eq!(app.sidebar.list_state.selected(), Some(0));

    // 2. List Pane navigation
    app.focus = Pane::List;
    app.handle_key(key(KeyCode::Char('2')));
    assert_eq!(app.selected_id, Some(second_id));
    assert_eq!(app.list_state.selected(), Some(1));

    app.handle_key(key(KeyCode::Char('1')));
    assert_eq!(app.selected_id, Some(first_id));
    assert_eq!(app.list_state.selected(), Some(0));

    // 3. Preview Pane fragment navigation
    // Add a second fragment to second snippet
    add_fragment(
        &app.library,
        &second_id.to_string(),
        &FragmentAddOptions {
            title: "script.sh".to_owned(),
            language: "bash".to_owned(),
            content: "echo second fragment\n".to_owned(),
            ..Default::default()
        },
    )
    .unwrap();
    app.rescan().unwrap();
    app.selected_id = Some(second_id);
    app.refresh_visible();
    app.focus = Pane::Preview;

    assert_eq!(app.fragment_index, 0);

    // Jump to 2nd fragment
    app.handle_key(key(KeyCode::Char('2')));
    assert_eq!(app.fragment_index, 1);

    // Jump back to 1st fragment
    app.handle_key(key(KeyCode::Char('1')));
    assert_eq!(app.fragment_index, 0);
}
