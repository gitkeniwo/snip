pub mod config;
pub mod folder_tag;
pub mod output;
pub mod query;
pub mod snippet;
pub mod system;
pub mod trash;

use snip::Library;
use snip::config::AppConfig;
use snip::error::{Result, SnipError};
use std::io::{self, IsTerminal};

pub use output::effective_output;
use output::{resolve_color, resolve_output};

use crate::cli::{Cli, Command};

pub fn run(cli: &Cli) -> Result<()> {
    if let Some(Command::Completion(args)) = &cli.command {
        return config::command_completion(args);
    }
    if let Some(Command::Config(args)) = &cli.command {
        return config::command_config(args, cli.output);
    }
    let config = AppConfig::load()?;
    let output = resolve_output(cli.output, &config);
    let color = resolve_color(cli.color, &config);
    if cli.command.is_none() {
        #[cfg(feature = "tui")]
        {
            if io::stdin().is_terminal() && io::stdout().is_terminal() {
                let path =
                    Library::discover(cli.library.as_deref(), config.default_library.as_deref())?;
                return snip::tui::run(Library::open(&path)?, &config);
            }
        }
        return Err(SnipError::usage(
            "a command is required when stdin or stdout is not a terminal; try --help",
        ));
    }
    match cli.command.as_ref() {
        Some(Command::Init(args)) => return config::command_init(args, output),
        Some(Command::Import(args)) => return system::command_import(args, output),
        _ => {}
    }
    let path = Library::discover(cli.library.as_deref(), config.default_library.as_deref())?;
    let library = Library::open(&path)?;
    let command = cli.command.as_ref().expect("command checked above");
    match command {
        #[cfg(feature = "tui")]
        Command::Tui => snip::tui::run(library, &config),
        Command::Info => query::command_info(&library, output),
        Command::List(args) => query::command_list(&library, args, output),
        Command::Open(args) => query::command_open(&library, args, output, &config),
        Command::Search(args) => query::command_search(&library, args, output),
        Command::Show(args) => query::command_show(&library, args, output),
        Command::Cat(args) => query::command_cat(&library, args),
        Command::Preview(args) => query::command_preview(&library, args, color, &config),
        Command::Path(args) => query::command_path(&library, args),
        Command::Create(args) => snippet::command_create(&library, args, output, &config),
        Command::Edit(args) => snippet::command_edit(&library, args, output, &config),
        Command::Fragment(args) => snippet::command_fragment(&library, args, output, &config),
        Command::Folder(args) => folder_tag::command_folder(&library, args, output),
        Command::Tag(args) => folder_tag::command_tag(&library, args, output),
        Command::Delete(args) => snippet::command_delete(&library, args, output),
        Command::Trash => trash::command_trash(&library, output),
        Command::Restore(args) => trash::command_restore(&library, args, output),
        Command::Purge(args) => trash::command_purge(&library, args, output),
        Command::Doctor(args) => system::command_doctor(&library, args, output),
        Command::Organize(args) => system::command_organize(&library, args, output),
        Command::Git(args) => system::command_git(&library, args),
        Command::Config(_) | Command::Init(_) | Command::Import(_) | Command::Completion(_) => {
            unreachable!()
        }
    }
}
