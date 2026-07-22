mod cli;
mod commands;

use clap::{CommandFactory, Parser};
use cli::{Cli, OutputMode};
use std::io::IsTerminal;

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
            eprintln!("snip: error: {error}");
        } else {
            let value = serde_json::json!({
                "error": {
                    "code": error.kind.code(),
                    "message": error.message,
                }
            });
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
