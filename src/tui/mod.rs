pub mod app;
pub mod clipboard;
pub mod editor;
pub mod event;
pub mod highlight;
pub mod icons;
pub mod layout;
pub mod modal;
pub mod preview;
pub mod selection;
pub mod sidebar;
pub mod snippet_list;
pub mod state;
pub mod theme;
pub mod trash;
pub mod ui;

use std::io::{self, IsTerminal, Stdout};
use std::panic;
use std::sync::Arc;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self as terminal_event, DisableMouseCapture, EnableMouseCapture, Event,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::config::AppConfig;
use crate::error::{Result, SnipError};
use crate::filesystem::Library;

use self::app::{App, Effect};
use self::clipboard::ClipboardMethod;
use self::editor::EditOutcome;
use self::state::StatusLevel;

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn run(library: Library, config: &AppConfig) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(SnipError::usage("the TUI requires an interactive terminal"));
    }
    let _panic_hook = PanicHookGuard::install();
    let mut guard = TerminalGuard::new()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new(library, config)?;
    let (_watcher, receiver) = event::start_watcher(app.library.root())?;

    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        let mut effects = Vec::new();
        if terminal_event::poll(Duration::from_millis(120))? {
            match terminal_event::read()? {
                Event::Key(key) if key.kind == ratatui::crossterm::event::KeyEventKind::Press => {
                    effects.extend(app.handle_key(key));
                }
                Event::Mouse(mouse) => effects.extend(app.handle_mouse(mouse)),
                _ => {}
            }
        }
        let mut dirty = false;
        while receiver.try_recv().is_ok() {
            dirty = true;
        }
        if dirty && let Err(error) = app.rescan() {
            app.set_status(error.to_string(), StatusLevel::Error);
        }
        app.tick_status();
        if let Err(error) = app.tick_theme() {
            app.set_status(error.to_string(), StatusLevel::Error);
        }
        for effect in effects {
            execute_effect(effect, &mut app, &mut terminal, &mut guard)?;
        }
    }
    Ok(())
}

fn execute_effect(
    effect: Effect,
    app: &mut App,
    terminal: &mut TuiTerminal,
    guard: &mut TerminalGuard,
) -> Result<()> {
    match effect {
        Effect::SpawnEditor(request) => {
            guard.suspend()?;
            let outcome =
                editor::run_external_edit(&app.library, request, app.editor_cmd.as_deref());
            guard.resume()?;
            *terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
            match outcome {
                Ok(outcome) => app.handle_editor_outcome(outcome),
                Err(error) => app.set_status(error.to_string(), StatusLevel::Error),
            }
        }
        Effect::ForceSave(request) => match editor::force_save(&app.library, &request) {
            Ok(()) => app.handle_editor_outcome(EditOutcome::Saved),
            Err(error) => app.set_status(error.to_string(), StatusLevel::Error),
        },
        Effect::CopyToClipboard { text, label } => match clipboard::copy(&text) {
            Ok(method) => {
                let method = match method {
                    ClipboardMethod::System => "system clipboard",
                    ClipboardMethod::Osc52 => "OSC 52",
                };
                app.set_status(
                    format!("copied {} B {label} ({method})", text.len()),
                    StatusLevel::Info,
                );
            }
            Err(error) => app.set_status(error.to_string(), StatusLevel::Error),
        },
    }
    Ok(())
}

struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        setup_terminal()?;
        Ok(Self { active: true })
    }

    fn suspend(&mut self) -> Result<()> {
        if self.active {
            restore_terminal();
            self.active = false;
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if !self.active {
            setup_terminal()?;
            self.active = true;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.active {
            restore_terminal();
        }
    }
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture) {
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        return Err(error.into());
    }
    Ok(())
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
}

struct PanicHookGuard {
    previous: Arc<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync + 'static>,
}

impl PanicHookGuard {
    fn install() -> Self {
        let previous =
            Arc::<dyn Fn(&panic::PanicHookInfo<'_>) + Send + Sync>::from(panic::take_hook());
        let hook = Arc::clone(&previous);
        panic::set_hook(Box::new(move |info| {
            restore_terminal();
            hook(info);
        }));
        Self { previous }
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        let previous = Arc::clone(&self.previous);
        panic::set_hook(Box::new(move |info| previous(info)));
    }
}
