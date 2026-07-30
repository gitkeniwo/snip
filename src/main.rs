mod cli;
mod commands;

use clap::{CommandFactory, Parser};
use cli::{Cli, ColorMode, OutputMode};
use std::io::IsTerminal;

fn render_hint(hint: &str) -> String {
    let mut lines = hint.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    std::iter::once(first.to_owned())
        .chain(lines.map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("      {line}")
            }
        }))
        .collect::<Vec<_>>()
        .join("\n")
}

fn use_error_color(cli: &Cli) -> bool {
    std::io::stderr().is_terminal()
        && cli.color != Some(ColorMode::Never)
        && std::env::var_os("NO_COLOR").is_none()
}

fn main() {
    let cli = Cli::parse();
    let output = commands::effective_output(&cli);
    if cli.command.is_none()
        && (!std::io::stdin().is_terminal() || !std::io::stdout().is_terminal())
        && output == OutputMode::Human
    {
        eprintln!("{}", Cli::command().render_help());
    }
    if let Err(error) = commands::run(&cli) {
        if output == OutputMode::Human {
            if use_error_color(&cli) {
                eprintln!("\x1b[1;31msnip: error:\x1b[0m {error}");
                if let Some(hint) = &error.hint {
                    eprintln!();
                    eprintln!("\x1b[1;36mhint:\x1b[0m {}", render_hint(hint));
                }
            } else {
                eprintln!("snip: error: {error}");
                if let Some(hint) = &error.hint {
                    eprintln!();
                    eprintln!("hint: {}", render_hint(hint));
                }
            }
        } else {
            let mut value = serde_json::json!({
                "error": { "code": error.kind.code(), "message": error.message }
            });
            if let Some(hint) = error.hint {
                value["error"]["hint"] = serde_json::Value::String(hint);
            }
            eprintln!(
                "{}",
                serde_json::to_string(&value).unwrap_or_else(|_| {
                    "{\"error\":{\"code\":\"internal\",\"message\":\"failed to encode error\"}}"
                        .to_owned()
                })
            );
        }
        std::process::exit(error.kind.exit_code());
    }
}

#[cfg(test)]
mod tests {
    use super::render_hint;

    #[test]
    fn hint_continuation_lines_align_with_hint_text() {
        assert_eq!(
            render_hint("first\n  second\n\nthird"),
            "first\n        second\n\n      third"
        );
    }
}
