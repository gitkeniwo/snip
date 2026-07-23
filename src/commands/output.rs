use serde::Serialize;
use serde_json::json;
use snip::config::{AppConfig, ColorSetting, OutputSetting};
use snip::domain::Snippet;
use snip::error::{Result, SnipError};
use std::io::Write;
use std::path::Path;
use std::process::{Command as ProcessCommand, Stdio};

use crate::cli::{Cli, ColorMode, OutputMode};

pub fn effective_output(cli: &Cli) -> OutputMode {
    cli.output.unwrap_or_else(|| {
        AppConfig::load()
            .ok()
            .as_ref()
            .map_or(OutputMode::Human, |config| resolve_output(None, config))
    })
}

pub fn resolve_output(explicit: Option<OutputMode>, config: &AppConfig) -> OutputMode {
    explicit.unwrap_or(match config.output {
        Some(OutputSetting::Json) => OutputMode::Json,
        Some(OutputSetting::Jsonl) => OutputMode::Jsonl,
        Some(OutputSetting::Human) | None => OutputMode::Human,
    })
}

pub fn resolve_color(explicit: Option<ColorMode>, config: &AppConfig) -> ColorMode {
    explicit.unwrap_or(match config.color {
        Some(ColorSetting::Always) => ColorMode::Always,
        Some(ColorSetting::Never) => ColorMode::Never,
        Some(ColorSetting::Auto) | None => ColorMode::Auto,
    })
}

pub fn send_to_pager(content: &str, configured_pager: Option<&str>) -> Result<()> {
    let pager = configured_pager
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| std::env::var("PAGER").unwrap_or_else(|_| "less -R".to_owned()));
    let parts = shlex::split(&pager)
        .filter(|parts| !parts.is_empty())
        .ok_or_else(|| SnipError::usage(format!("invalid pager command: {pager:?}")))?;
    let mut child = ProcessCommand::new(&parts[0])
        .args(&parts[1..])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| SnipError::io(format!("cannot start pager: {error}")))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| SnipError::io("pager stdin is unavailable"))?
        .write_all(content.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        return Err(SnipError::io(format!("pager exited with status {status}")));
    }
    Ok(())
}

pub fn print_mutation(
    snippet: &Snippet,
    changes: Option<&snip::ChangeSet>,
    output: OutputMode,
) -> Result<()> {
    if output == OutputMode::Human {
        println!("updated: {}", snippet.package_path.display());
        println!("id: {}", snippet.id);
        println!("fingerprint: {}", snippet.fingerprint);
        if let Some(changes) = changes {
            println!("fields: {}", changes.fields.join(", "));
        }
    } else {
        print_record(
            &json!({
                "snippet": snippet,
                "changes": changes,
            }),
            output,
        )?;
    }
    Ok(())
}

pub fn print_record<T: Serialize>(value: &T, output: OutputMode) -> Result<()> {
    match output {
        OutputMode::Human => unreachable!(),
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(value)?),
        OutputMode::Jsonl => println!("{}", serde_json::to_string(value)?),
    }
    Ok(())
}

pub fn print_records<T: Serialize>(values: &[T], output: OutputMode) -> Result<()> {
    match output {
        OutputMode::Human => unreachable!(),
        OutputMode::Json => println!("{}", serde_json::to_string_pretty(values)?),
        OutputMode::Jsonl => {
            for value in values {
                println!("{}", serde_json::to_string(value)?);
            }
        }
    }
    Ok(())
}

pub fn print_simple_path(label: &str, path: &Path, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("{label}: {}", path.display());
    } else {
        print_record(&json!({label: path}), output)?;
    }
    Ok(())
}

pub fn print_count(label: &str, value: usize, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("{label}: {value}");
    } else {
        print_record(&json!({label: value}), output)?;
    }
    Ok(())
}

pub fn display_json_scalar(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
