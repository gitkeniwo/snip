mod cli;
mod commands;

use clap::Parser;
use cli::{Cli, OutputMode};

fn main() {
    let cli = Cli::parse();
    let output = commands::effective_output(&cli);
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
