use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use snip::Library;
use snip::config::{AppConfig, config_path};
use snip::error::{Result, SnipError};

use crate::cli::{Cli, ColorMode, InitArgs, OutputMode};

pub struct Outcome {
    pub library: Library,
}

enum Start {
    Choose,
    Create,
}

/// Runs the interactive setup. Returns `Ok(None)` when the user cancels.
pub fn run(config: &AppConfig, color: ColorMode) -> Result<Option<Outcome>> {
    run_with(config, color, Start::Choose, None, false)
}

pub fn run_init(config: &AppConfig, args: &InitArgs, color: ColorMode) -> Result<Option<Outcome>> {
    run_with(config, color, Start::Create, args.name.as_deref(), args.git)
}

/// True when a wizard is allowed to run at all.
pub fn is_interactive(cli: &Cli, output: OutputMode) -> bool {
    // `Cli::library` includes SNIP_LIBRARY through clap's `env` declaration.
    output == OutputMode::Human
        && io::stdin().is_terminal()
        && io::stdout().is_terminal()
        && cli.library.is_none()
}

pub fn is_init_interactive(args: &InitArgs, output: OutputMode) -> bool {
    args.path.is_none()
        && !args.yes
        && output == OutputMode::Human
        && io::stdin().is_terminal()
        && io::stdout().is_terminal()
}

fn run_with(
    config: &AppConfig,
    color: ColorMode,
    start: Start,
    supplied_name: Option<&str>,
    supplied_git: bool,
) -> Result<Option<Outcome>> {
    let color = color_enabled(color);
    println!(
        "\n  snip — set up your library\n\n  A library is an ordinary folder of files. You can move it, sync it,\n  or put it in Git at any time.\n"
    );

    let create = match start {
        Start::Create => true,
        Start::Choose => {
            println!("  1  create a new library\n  2  connect to a library I already have\n");
            match ask_choice_stdio("choice", &["create", "connect"], 0, color)? {
                Some(choice) => choice == 0,
                None => return cancelled(),
            }
        }
    };

    if create {
        create_library(config, color, supplied_name, supplied_git)
    } else {
        connect_library(config, color)
    }
}

fn create_library(
    config: &AppConfig,
    color: bool,
    supplied_name: Option<&str>,
    supplied_git: bool,
) -> Result<Option<Outcome>> {
    let path = match ask_line_stdio("path", Some("~/Main.sniplib"), color)? {
        Some(path) => expand_user_path(&path)?,
        None => return cancelled(),
    };
    if path.join("snip.toml").is_file() {
        println!("  that path already holds a snip library — connecting to it instead.");
        return finish_connected(config, color, &path);
    }
    let default_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Main");
    let name = match supplied_name {
        Some(name) => name.to_owned(),
        None => match ask_line_stdio("name", Some(default_name), color)? {
            Some(name) => name,
            None => return cancelled(),
        },
    };
    let set_as_default = match ask_default(config, color)? {
        Some(answer) => answer,
        None => return cancelled(),
    };
    let git_initialized = if supplied_git {
        true
    } else {
        match ask_yes_no_stdio("initialize a Git repo inside it?", false, color)? {
            Some(answer) => answer,
            None => return cancelled(),
        }
    };
    let library = Library::init(&path, Some(&name))?;
    if git_initialized {
        snip::git::init(library.root())?;
    }
    finish(config, library, set_as_default, git_initialized, true)
}

fn connect_library(config: &AppConfig, color: bool) -> Result<Option<Outcome>> {
    loop {
        let path = match ask_line_stdio("path to your .sniplib", None, color)? {
            Some(path) => expand_user_path(&path)?,
            None => return cancelled(),
        };
        if !path.join("snip.toml").is_file() {
            let display_path = absolute_path(&path)?;
            println!(
                "  no snip library there — {} has no snip.toml inside.",
                display_path.display()
            );
            continue;
        }
        let library = Library::open(&path)?;
        let set_as_default = match ask_default(config, color)? {
            Some(answer) => answer,
            None => return cancelled(),
        };
        return finish(config, library, set_as_default, false, false);
    }
}

fn finish_connected(config: &AppConfig, color: bool, path: &Path) -> Result<Option<Outcome>> {
    let library = Library::open(path)?;
    let set_as_default = match ask_default(config, color)? {
        Some(answer) => answer,
        None => return cancelled(),
    };
    finish(config, library, set_as_default, false, false)
}

fn ask_default(config: &AppConfig, color: bool) -> Result<Option<bool>> {
    if let Some(current) = &config.default_library {
        println!("  current default: {}", current.display());
        ask_yes_no_stdio("replace your current default library?", false, color)
    } else {
        ask_yes_no_stdio("set as snip's default library?", true, color)
    }
}

fn finish(
    config: &AppConfig,
    library: Library,
    set_as_default: bool,
    git_initialized: bool,
    created: bool,
) -> Result<Option<Outcome>> {
    if set_as_default {
        let mut updated = config.clone();
        updated.default_library = Some(library.root().to_path_buf());
        updated.save()?;
    }
    println!();
    println!(
        "  {:<9} {}",
        if created { "created" } else { "connected" },
        library.root().display()
    );
    if set_as_default {
        println!("  {:<9} written to {}", "default", config_path()?.display());
    } else {
        println!(
            "  {:<9} not changed — pass --library or set SNIP_LIBRARY to use this library",
            "default"
        );
    }
    if git_initialized {
        println!("  {:<9} initialized", "git");
    }
    println!();
    Ok(Some(Outcome { library }))
}

pub fn print_init_next_steps() {
    println!(
        "  {:<9} snip create --title \"First snippet\"\n            snip      open the browser\n",
        "next"
    );
}

fn cancelled<T>() -> Result<Option<T>> {
    println!("\n  cancelled — nothing was created.");
    Ok(None)
}

fn color_enabled(color: ColorMode) -> bool {
    color != ColorMode::Never && env::var_os("NO_COLOR").is_none()
}

fn prompt(label: &str, default: Option<&str>, color: bool) -> String {
    if color {
        match default {
            Some(default) => {
                format!("  \x1b[36m❯\x1b[0m {label} \x1b[2m[{default}]\x1b[0m: ")
            }
            None => format!("  \x1b[36m❯\x1b[0m {label}: "),
        }
    } else {
        match default {
            Some(default) => format!("  ❯ {label} [{default}]: "),
            None => format!("  ❯ {label}: "),
        }
    }
}

fn ask_line_stdio(label: &str, default: Option<&str>, color: bool) -> Result<Option<String>> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    ask_line_from(&mut input, label, default, color, &mut io::stdout())
}

fn ask_choice_stdio(
    label: &str,
    options: &[&str],
    default: usize,
    color: bool,
) -> Result<Option<usize>> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    ask_choice_from(
        &mut input,
        label,
        options,
        default,
        color,
        &mut io::stdout(),
    )
}

fn ask_yes_no_stdio(label: &str, default: bool, color: bool) -> Result<Option<bool>> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    ask_yes_no_from(&mut input, label, default, color, &mut io::stdout())
}

fn ask_line_from(
    input: &mut dyn BufRead,
    label: &str,
    default: Option<&str>,
    color: bool,
    output: &mut dyn Write,
) -> Result<Option<String>> {
    write!(output, "{}", prompt(label, default, color))?;
    output.flush()?;
    let mut answer = String::new();
    if input.read_line(&mut answer)? == 0 {
        return Ok(None);
    }
    let answer = answer.trim();
    Ok(Some(match (answer.is_empty(), default) {
        (true, Some(default)) => default.to_owned(),
        _ => answer.to_owned(),
    }))
}

fn ask_choice_from(
    input: &mut dyn BufRead,
    label: &str,
    options: &[&str],
    default: usize,
    color: bool,
    output: &mut dyn Write,
) -> Result<Option<usize>> {
    loop {
        let default = (default + 1).to_string();
        let Some(answer) = ask_line_from(input, label, Some(&default), color, output)? else {
            return Ok(None);
        };
        if let Ok(number) = answer.parse::<usize>()
            && (1..=options.len()).contains(&number)
        {
            return Ok(Some(number - 1));
        }
    }
}

fn ask_yes_no_from(
    input: &mut dyn BufRead,
    label: &str,
    default: bool,
    color: bool,
    output: &mut dyn Write,
) -> Result<Option<bool>> {
    let display = if default { "Y/n" } else { "y/N" };
    loop {
        write!(output, "{}", prompt(label, Some(display), color))?;
        output.flush()?;
        let mut answer = String::new();
        if input.read_line(&mut answer)? == 0 {
            return Ok(None);
        }
        let answer = answer.trim();
        if answer.is_empty() {
            return Ok(Some(default));
        }
        match answer.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(Some(true)),
            "n" | "no" => return Ok(Some(false)),
            _ => {}
        }
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn expand_user_path(value: &str) -> Result<PathBuf> {
    if let Some(rest) = value.strip_prefix("~/") {
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .ok_or_else(|| SnipError::io("cannot locate home directory"))?;
        Ok(PathBuf::from(home).join(rest))
    } else {
        Ok(PathBuf::from(value))
    }
}

#[cfg(test)]
mod tests {
    use super::{ask_choice_from, ask_line_from, ask_yes_no_from, prompt};
    use std::io::Cursor;

    #[test]
    fn line_uses_default_and_eof_cancels() {
        let mut output = Vec::new();
        assert_eq!(
            ask_line_from(
                &mut Cursor::new(b"\n"),
                "path",
                Some("Main"),
                false,
                &mut output
            )
            .unwrap(),
            Some("Main".to_owned())
        );
        assert_eq!(
            ask_line_from(
                &mut Cursor::new(b""),
                "path",
                Some("Main"),
                false,
                &mut output
            )
            .unwrap(),
            None
        );
        assert_eq!(
            prompt("path to your .sniplib", None, false),
            "  ❯ path to your .sniplib: "
        );
    }

    #[test]
    fn yes_no_and_choices_validate_input() {
        let mut output = Vec::new();
        assert_eq!(
            ask_yes_no_from(&mut Cursor::new(b"y\n"), "git", false, false, &mut output).unwrap(),
            Some(true)
        );
        assert_eq!(
            ask_yes_no_from(&mut Cursor::new(b"N\n"), "git", true, false, &mut output).unwrap(),
            Some(false)
        );
        assert_eq!(
            ask_yes_no_from(&mut Cursor::new(b"\n"), "git", false, false, &mut output).unwrap(),
            Some(false)
        );
        assert_eq!(
            ask_choice_from(
                &mut Cursor::new(b"bad\n2\n"),
                "choice",
                &["one", "two"],
                0,
                false,
                &mut output
            )
            .unwrap(),
            Some(1)
        );
    }
}
