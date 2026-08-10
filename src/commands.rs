pub mod config;
pub mod folder_tag;
pub mod gist;
mod install;
#[cfg(feature = "tui")]
pub mod keys;
pub mod man;
mod man_pages;
pub mod onboard;
pub mod output;
pub mod query;
pub mod snippet;
pub mod system;
pub mod theme;
pub mod trash;

use snip::Library;
use snip::config::AppConfig;
use snip::error::{ErrorKind, Result, SnipError};
#[cfg(feature = "tui")]
use std::io::{self, IsTerminal};

pub use output::effective_output;
use output::{resolve_color, resolve_output};

use crate::cli::{Cli, Command, GitArgs, GitCommand, OutputMode};

#[cfg(feature = "tui")]
fn tui_config_with_cli_override(mut config: AppConfig, simplified_ui: Option<bool>) -> AppConfig {
    if let Some(simplified_ui) = simplified_ui {
        config
            .tui
            .get_or_insert_with(snip::config::TuiConfig::default)
            .simplified_ui = simplified_ui;
    }
    config
}

pub fn run(cli: &Cli) -> Result<()> {
    if let Some(Command::Completion(args)) = &cli.command {
        return config::command_completion(args);
    }
    if let Some(Command::Config(args)) = &cli.command {
        return config::command_config(args, cli.output);
    }
    if let Some(Command::Man(args)) = &cli.command {
        return man::command_man(args, cli.output);
    }
    if let Some(Command::Theme(args)) = &cli.command {
        return theme::command_theme(args, cli.output);
    }
    #[cfg(feature = "tui")]
    if let Some(Command::Keys(args)) = &cli.command {
        return keys::command_keys(args, cli.output);
    }
    let config = AppConfig::load()?;
    let output = resolve_output(cli.output, &config);
    let color = resolve_color(cli.color, &config);
    if cli.command.is_none() {
        #[cfg(feature = "tui")]
        {
            if io::stdin().is_terminal() && io::stdout().is_terminal() {
                let library = open_library_or_onboard(cli, &config, output, color, true)?;
                let config = tui_config_with_cli_override(AppConfig::load()?, cli.simplified_ui);
                return snip::tui::run(library, &config);
            }
        }
        return Err(SnipError::usage(
            "a command is required when stdin or stdout is not a terminal; try --help",
        ));
    }
    match cli.command.as_ref() {
        Some(Command::Init(args)) => return config::command_init(args, output, cli.color),
        Some(Command::Import(args)) => return system::command_import(args, output),
        Some(Command::Git(GitArgs {
            command:
                GitCommand::Clone {
                    remote,
                    path,
                    gh,
                    set_default,
                },
        })) => {
            return system::command_git_clone(remote, path.as_deref(), *gh, *set_default, output);
        }
        _ => {}
    }
    let command = cli.command.as_ref().expect("command checked above");
    #[cfg(feature = "tui")]
    let allow_wizard = matches!(command, Command::Tui);
    #[cfg(not(feature = "tui"))]
    let allow_wizard = false;
    let library = open_library_or_onboard(cli, &config, output, color, allow_wizard)?;
    match command {
        #[cfg(feature = "tui")]
        Command::Tui => {
            let config = tui_config_with_cli_override(AppConfig::load()?, cli.simplified_ui);
            snip::tui::run(library, &config)
        }
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
        Command::Git(args) => system::command_git(&library, args, output),
        Command::Gist(args) => gist::command_gist(&library, args, output),
        Command::Config(_)
        | Command::Init(_)
        | Command::Import(_)
        | Command::Man(_)
        | Command::Theme(_)
        | Command::Completion(_) => {
            unreachable!()
        }
        #[cfg(feature = "tui")]
        Command::Keys(_) => unreachable!(),
    }
}

fn open_library_or_onboard(
    cli: &Cli,
    config: &AppConfig,
    output: OutputMode,
    color: crate::cli::ColorMode,
    allow_wizard: bool,
) -> Result<Library> {
    match Library::discover(cli.library.as_deref(), config.default_library.as_deref()) {
        Ok(path) => Library::open(&path),
        Err(error)
            if allow_wizard
                && error.kind == ErrorKind::NoLibrary
                && onboard::is_interactive(cli, output) =>
        {
            let outcome = onboard::run(config, color)?;
            match outcome {
                Some(outcome) => {
                    println!("  opening snip…\n");
                    Ok(outcome.library)
                }
                None => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(all(test, feature = "tui"))]
mod tests {
    use super::tui_config_with_cli_override;
    use snip::config::{AppConfig, TuiConfig};

    #[test]
    fn simplified_ui_cli_override_takes_precedence_over_config() {
        let configured = AppConfig {
            tui: Some(TuiConfig {
                simplified_ui: true,
                ..TuiConfig::default()
            }),
            ..AppConfig::default()
        };

        let inherited = tui_config_with_cli_override(configured.clone(), None);
        assert!(inherited.tui.unwrap().simplified_ui);

        let forced_powerline = tui_config_with_cli_override(configured, Some(false));
        assert!(!forced_powerline.tui.unwrap().simplified_ui);

        let forced_simplified = tui_config_with_cli_override(AppConfig::default(), Some(true));
        assert!(forced_simplified.tui.unwrap().simplified_ui);
    }
}
