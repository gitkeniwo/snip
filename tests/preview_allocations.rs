#![cfg(feature = "tui")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use snip::service::{CreateOptions, create_snippet};
use snip::tui::app::App;
use snip::tui::selection::SelectionKey;
use snip::{AppConfig, Library};

struct CountingAllocator;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[test]
fn cached_preview_frames_keep_allocations_bounded() {
    const FRAMES: usize = 20;

    let temporary = tempfile::tempdir_in(".").unwrap();
    let library = Library::init(&temporary.path().join("Allocations.sniplib"), None).unwrap();
    let content = (0..3_000)
        .map(|index| format!("let value_{index} = {index};"))
        .collect::<Vec<_>>()
        .join("\n");
    create_snippet(
        &library,
        &CreateOptions {
            title: "Large Rust snippet".to_owned(),
            language: "rust".to_owned(),
            content,
            ..CreateOptions::default()
        },
    )
    .unwrap();
    let mut app = App::new(library, &AppConfig::default()).unwrap();
    let snippet = app.selected_snippet().cloned().unwrap();
    let (_, rebuilt) = app
        .preview
        .get(
            &snippet,
            app.fragment_index,
            80,
            app.show_line_numbers,
            &app.highlighter,
            app.theme,
        )
        .unwrap();
    assert!(rebuilt);
    let (_, rebuilt) = app
        .preview
        .get(
            &snippet,
            app.fragment_index,
            80,
            app.show_line_numbers,
            &app.highlighter,
            app.theme,
        )
        .unwrap();
    assert!(!rebuilt);
    let (_, rebuilt) = app
        .preview
        .get(
            &snippet,
            app.fragment_index,
            81,
            app.show_line_numbers,
            &app.highlighter,
            app.theme,
        )
        .unwrap();
    assert!(rebuilt, "content width must be part of the cache key");
    let (_, rebuilt) = app
        .preview
        .get(
            &snippet,
            app.fragment_index,
            81,
            !app.show_line_numbers,
            &app.highlighter,
            app.theme,
        )
        .unwrap();
    assert!(
        rebuilt,
        "line-number visibility must be part of the cache key"
    );

    let mut changed_theme = app.theme;
    changed_theme.accent = ratatui::style::Color::Red;
    let (_, rebuilt) = app
        .preview
        .get(
            &snippet,
            app.fragment_index,
            81,
            !app.show_line_numbers,
            &app.highlighter,
            changed_theme,
        )
        .unwrap();
    assert!(
        !rebuilt,
        "themes use explicit invalidation instead of the cache key"
    );
    app.preview.invalidate();
    let (_, rebuilt) = app
        .preview
        .get(
            &snippet,
            app.fragment_index,
            80,
            app.show_line_numbers,
            &app.highlighter,
            app.theme,
        )
        .unwrap();
    assert!(rebuilt, "explicit invalidation must rebuild the preview");

    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for _ in 0..FRAMES {
        app.preview
            .get(
                &snippet,
                app.fragment_index,
                80,
                app.show_line_numbers,
                &app.highlighter,
                app.theme,
            )
            .unwrap();
    }
    COUNTING.store(false, Ordering::Relaxed);
    let allocations = ALLOCATIONS.load(Ordering::Relaxed);

    assert!(
        allocations < 100,
        "{FRAMES} cached preview frames allocated {allocations} times"
    );

    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            snip::tui::preview::draw_preview(frame, &mut app, area);
        })
        .unwrap();

    // Force a cache hit with a different selection key. This covers the
    // `prepare` branch without rebuilding the preview, then the repeated
    // frames below cover `reclamp` and the borrowed per-Line rendering path.
    app.preview_selection.prepare(
        SelectionKey {
            snippet_id: uuid::Uuid::nil(),
            fragment_index: app.fragment_index,
            fingerprint: snippet.fingerprint.0.clone(),
        },
        Vec::new(),
    );
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    terminal
        .draw(|frame| {
            let area = frame.area();
            snip::tui::preview::draw_preview(frame, &mut app, area);
        })
        .unwrap();
    COUNTING.store(false, Ordering::Relaxed);
    let prepare_allocations = ALLOCATIONS.load(Ordering::Relaxed);

    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for _ in 0..FRAMES {
        terminal
            .draw(|frame| {
                let area = frame.area();
                snip::tui::preview::draw_preview(frame, &mut app, area);
            })
            .unwrap();
    }
    COUNTING.store(false, Ordering::Relaxed);
    let draw_allocations = ALLOCATIONS.load(Ordering::Relaxed);

    assert!(
        prepare_allocations < 5_000,
        "the cached prepare draw allocated {prepare_allocations} times"
    );
    assert!(
        draw_allocations < 32_000,
        "{FRAMES} cached preview draws allocated {draw_allocations} times"
    );
}
