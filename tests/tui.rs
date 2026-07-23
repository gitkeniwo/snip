#![cfg(feature = "tui")]

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use snip::service::{
    CreateOptions, EditOptions, FragmentAddOptions, add_fragment, create_snippet, edit_snippet,
};
use snip::tui::app::{App, Effect};
use snip::tui::editor::{EditOutcome, EditTarget, force_save};
use snip::tui::highlight::Highlighter;
use snip::tui::icons::IconMode;
use snip::tui::modal::{Modal, ModalAction};
use snip::tui::state::{Pane, SidebarItem, SortMode};
use snip::tui::theme::{Appearance, TuiTheme};
use snip::{AppConfig, Library, TuiConfig, TuiIconSetting, TuiSortSetting, TuiThemeSetting};
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
    assert!(rendered.contains("1/2 · 1/1"));
    assert!(rendered.contains("Tab"));
    assert!(rendered.contains('/'));
    assert!(rendered.contains("001-Alpha Rust.rs rs"));
    assert!(rendered.contains("1│ fn alpha() {}"));
    let buffer = terminal.backend().buffer();
    let bottom = row_text(buffer, 29);
    assert!(bottom.starts_with('\u{e0b6}'));
    assert!(bottom.ends_with('\u{e0b4}'));
    assert!(bottom.find("←/→").unwrap() < 10);
    assert!(
        bottom.rfind("new").unwrap() > 60,
        "pane-specific actions should be grouped on the right"
    );
    let nav_key_x = text_column(&bottom, "←/→");
    assert_eq!(
        buffer.cell((nav_key_x, 29)).unwrap().bg,
        app.theme.pill_primary
    );
    let nav_join_x = nav_key_x + 4;
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
    let action_x = text_column_from_end(&bottom, "new");
    assert_eq!(
        buffer.cell((action_x, 29)).unwrap().bg,
        app.theme.pill_secondary
    );
    let top = row_text(buffer, 0);
    assert!(top.starts_with('\u{e0b6}'));
    assert!(top.ends_with('\u{e0b4}'));
    let brand_x = text_column(&top, "snip");
    let breadcrumb_x = text_column(&top, "~");
    let counts_x = text_column_from_end(&top, "1/2 · 1/1");
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
    assert_eq!(buffer.cell((29, 2)).unwrap().symbol(), "A");
    assert_eq!(buffer.cell((29, 3)).unwrap().symbol(), "[");
    assert_eq!(buffer.cell((27, 3)).unwrap().symbol(), "★");
    assert!(row_text_from(buffer, 3, 29).starts_with("[Code > Rust]"));
    assert_eq!(
        buffer.cell((2, 7)).unwrap().symbol(),
        "#",
        "top-level tags should not inherit the folder icon gutter"
    );
    assert_eq!(buffer.cell((2, 1)).unwrap().symbol(), "L");
    assert_eq!(buffer.cell((2, 3)).unwrap().symbol(), "▾");
    assert_eq!(buffer.cell((2, 6)).unwrap().symbol(), "T");
    assert_eq!(buffer.cell((2, 6)).unwrap().fg, app.theme.tag);
    assert_eq!(buffer.cell((3, 6)).unwrap().symbol(), "a");
    assert_eq!(buffer.cell((2, 7)).unwrap().symbol(), "#");
    assert_eq!(buffer.cell((3, 7)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((56, 1)).unwrap().symbol(), "P");
    assert_eq!(buffer.cell((56, 2)).unwrap().symbol(), "A");
    assert_eq!(buffer.cell((56, 3)).unwrap().symbol(), "C");
    assert_eq!(buffer.cell((56, 4)).unwrap().symbol(), "#");
    let preview_bottom = row_text_from(buffer, 28, 54);
    assert!(preview_bottom.contains("Rust"));
    assert!(preview_bottom.contains("1 line"));

    let metadata = row_text_from(buffer, 3, 54);
    let tags = row_text_from(buffer, 4, 54);
    assert!(metadata.contains("Code/Rust · "));
    assert!(!metadata.contains("#dev"));
    assert!(metadata.contains("001-Alpha Rust.rs rs"));
    assert!(
        tags.contains("#dev"),
        "preview tags belong on their own row"
    );
    let filename_x = 54 + metadata.find("001-Alpha Rust.rs rs").unwrap() as u16;
    assert_ne!(
        buffer.cell((filename_x, 3)).unwrap().bg,
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
    assert!(rendered.contains("NAVIGATION"));
    assert!(rendered.contains("SNIPPETS"));
    assert!(rendered.contains("LIBRARY & GLOBAL"));
    assert!(rendered.contains("PREVIEW & MOUSE"));
    for label in [
        "Help",
        "snip TUI",
        "NAVIGATION",
        "SNIPPETS",
        "LIBRARY & GLOBAL",
        "PREVIEW & MOUSE",
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
    assert!(rendered.contains("first item"));
    assert!(rendered.contains("last item"));
    assert!(rendered.contains("rename snippet"));
    assert!(rendered.contains("move snippet"));
    assert!(rendered.contains("edit tags"));
    let tab_y = (0..buffer.area.height)
        .find(|&y| text_x(buffer, y, "next pane").is_some())
        .unwrap();
    assert!(text_x(buffer, tab_y, "Tab").unwrap() >= 13);
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

    let _ = app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 8, 3));
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
        .position(|folder| *folder == "Code/Shell")
        .unwrap();
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.selected_snippet().unwrap().folder, "Code/Shell");

    let pinned = app.selected_snippet().unwrap().pinned;
    app.handle_key(key(KeyCode::Char('p')));
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
    let Some(Modal::Input(language)) = app.modal.as_ref() else {
        panic!("expected language input");
    };
    assert_eq!(language.value, "python");
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
fn tui_config_controls_theme_sort_and_portable_icon_fallback() {
    let (_temporary, library, _first_id, _second_id) = fixture();
    let config = AppConfig {
        tui: Some(TuiConfig {
            theme: TuiThemeSetting::Light,
            sort: TuiSortSetting::Title,
            icons: TuiIconSetting::Nerd,
            ..TuiConfig::default()
        }),
        ..AppConfig::default()
    };
    let app = App::new(library, &config).unwrap();
    assert_eq!(app.theme.appearance, Appearance::Light);
    assert_eq!(app.sort, SortMode::Title);
    assert_eq!(app.icon_mode, IconMode::Ascii);
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
