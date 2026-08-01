use std::fs;
use std::io::{self, IsTerminal};

use serde::Serialize;
use serde_json::json;
use snip::config::{AppConfig, TuiConfig};
use snip::error::{Result, SnipError};
use snip::theme::validate::{self, Level};
use snip::theme::{Appearance, NamedColor, Theme, ThemeColor, ThemeSummary};

use super::output::{print_record, print_records, resolve_color, resolve_output};
use crate::cli::{AppearanceArg, ColorMode, OutputMode, ThemeArgs, ThemeCommand};

#[derive(Serialize)]
struct ListRecord {
    name: String,
    display_name: String,
    appearance: Appearance,
    source: Option<String>,
    builtin: bool,
    active: bool,
    error: Option<String>,
}

pub fn command_theme(args: &ThemeArgs, explicit_output: Option<OutputMode>) -> Result<()> {
    let config = AppConfig::load()?;
    let output = resolve_output(explicit_output, &config);
    match &args.command {
        ThemeCommand::List { appearance } => command_list(*appearance, &config, output),
        ThemeCommand::Show { name } => command_show(name, &config, output),
        ThemeCommand::Check { name } => command_check(name, output),
        ThemeCommand::Path => {
            println!("{}", snip::theme::themes_dir()?.display());
            Ok(())
        }
        ThemeCommand::Export { name, r#as, force } => {
            command_export(name, r#as.as_deref(), *force, output)
        }
        ThemeCommand::Use { name, appearance } => command_use(name, *appearance, &config, output),
    }
}

fn command_list(
    filter: Option<AppearanceArg>,
    config: &AppConfig,
    output: OutputMode,
) -> Result<()> {
    let tui = config.tui.clone().unwrap_or_default();
    let records = snip::theme::list()
        .into_iter()
        .filter(|summary| filter.is_none_or(|filter| appearance(filter) == summary.appearance))
        .map(|summary| list_record(summary, &tui))
        .collect::<Vec<_>>();
    if output != OutputMode::Human {
        return print_records(&records, output);
    }
    let mut current = None;
    for record in records {
        if current != Some(record.appearance) {
            if current.is_some() {
                println!();
            }
            println!("{}:", record.appearance.as_str());
            current = Some(record.appearance);
        }
        let suffix = if let Some(error) = &record.error {
            format!("  (invalid: {error})")
        } else if record.active {
            "  (active)".to_owned()
        } else {
            String::new()
        };
        println!("  {:<20} {:<24}{suffix}", record.name, record.display_name);
    }
    Ok(())
}

fn list_record(summary: ThemeSummary, config: &TuiConfig) -> ListRecord {
    let active_name = match summary.appearance {
        Appearance::Light => config.light_theme.as_deref().unwrap_or("light-default"),
        Appearance::Dark => config.dark_theme.as_deref().unwrap_or("dark-default"),
    };
    ListRecord {
        active: summary.name == active_name,
        name: summary.name,
        display_name: summary.display_name,
        appearance: summary.appearance,
        source: summary.source,
        builtin: summary.builtin,
        error: summary.error,
    }
}

fn command_show(name: &str, config: &AppConfig, output: OutputMode) -> Result<()> {
    let theme = snip::theme::load(name)?;
    if output != OutputMode::Human {
        return print_record(&theme, output);
    }
    println!("{} ({})", theme.display_name, theme.name);
    println!("appearance  {}", theme.appearance.as_str());
    let color = resolve_color(None, config);
    let swatches =
        color == ColorMode::Always || color == ColorMode::Auto && io::stdout().is_terminal();
    for (role, value) in ui_colors(&theme) {
        if swatches {
            print!("{}  \x1b[0m ", background_escape(value));
        }
        println!("{role:<16} {value}");
    }
    Ok(())
}

fn command_check(name: &str, output: OutputMode) -> Result<()> {
    let theme = snip::theme::load(name)?;
    let checks = validate::check(&theme);
    let ok = !checks.iter().any(|check| check.level == Level::Fail);
    if output == OutputMode::Human {
        println!("theme: {name}");
        for check in &checks {
            println!(
                "  {:<5} {:<24} {}",
                level_name(check.level),
                check.id,
                check.detail
            );
        }
    } else {
        print_record(&json!({ "name": name, "ok": ok, "checks": checks }), output)?;
    }
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

fn command_use(
    name: &str,
    slot: Option<AppearanceArg>,
    original: &AppConfig,
    output: OutputMode,
) -> Result<()> {
    let theme = snip::theme::load(name)?;
    let selected = slot.map(appearance).unwrap_or(theme.appearance);
    if slot.is_some() && selected != theme.appearance {
        eprintln!(
            "warning: theme {name} is a {} theme in the {} slot",
            theme.appearance.as_str(),
            selected.as_str()
        );
    }
    let mut config = original.clone();
    let tui = config.tui.get_or_insert_with(TuiConfig::default);
    match selected {
        Appearance::Light => tui.light_theme = Some(name.to_owned()),
        Appearance::Dark => tui.dark_theme = Some(name.to_owned()),
    }
    config.save()?;
    if output == OutputMode::Human {
        println!("theme: {name}");
    } else {
        print_record(
            &json!({ "updated": "theme", "name": name, "appearance": selected }),
            output,
        )?;
    }
    Ok(())
}

fn command_export(
    name: &str,
    save_as: Option<&str>,
    force: bool,
    output: OutputMode,
) -> Result<()> {
    let mut theme = snip::theme::load(name)?;
    let new_name = save_as
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{name}-custom"));
    snip::theme::validate_theme_name(&new_name)?;
    theme.name = new_name.clone();
    let directory = snip::theme::themes_dir()?;
    let path = directory.join(format!("{new_name}.toml"));
    if path.exists() && !force {
        return Err(SnipError::validation(format!(
            "{} already exists; pass --force to overwrite",
            path.display()
        )));
    }
    fs::create_dir_all(&directory).map_err(|error| {
        SnipError::io(format!("cannot create {}: {error}", directory.display()))
    })?;
    fs::write(&path, toml::to_string_pretty(&theme)?)
        .map_err(|error| SnipError::io(format!("cannot write {}: {error}", path.display())))?;
    if output == OutputMode::Human {
        println!("exported: {}", path.display());
    } else {
        print_record(&json!({ "name": new_name, "path": path }), output)?;
    }
    Ok(())
}

fn appearance(value: AppearanceArg) -> Appearance {
    match value {
        AppearanceArg::Light => Appearance::Light,
        AppearanceArg::Dark => Appearance::Dark,
    }
}

fn level_name(level: Level) -> &'static str {
    match level {
        Level::Ok => "ok",
        Level::Note => "note",
        Level::Warn => "warn",
        Level::Fail => "fail",
    }
}

fn ui_colors(theme: &Theme) -> [(&'static str, ThemeColor); 18] {
    let ui = &theme.ui;
    [
        ("background", ui.background),
        ("foreground", ui.foreground),
        ("accent", ui.accent),
        ("accent_alt", ui.accent_alt),
        ("border", ui.border),
        ("muted", ui.muted),
        ("selection_bg", ui.selection_bg),
        ("selection_fg", ui.selection_fg),
        ("retained_bg", ui.retained_bg),
        ("pill_primary", ui.pill_primary),
        ("pill_secondary", ui.pill_secondary),
        ("bar_bg", ui.bar_bg),
        ("bar_fg", ui.bar_fg),
        ("tag", ui.tag),
        ("rule", ui.rule),
        ("success", ui.success),
        ("warning", ui.warning),
        ("error", ui.error),
    ]
}

fn background_escape(color: ThemeColor) -> String {
    match color {
        ThemeColor::Rgb(red, green, blue) => format!("\x1b[48;2;{red};{green};{blue}m"),
        ThemeColor::Indexed(index) => format!("\x1b[48;5;{index}m"),
        ThemeColor::Named(named) => format!("\x1b[{}m", named_background_code(named)),
        ThemeColor::Terminal => "\x1b[49m".to_owned(),
    }
}

fn named_background_code(color: NamedColor) -> u8 {
    match color {
        NamedColor::Black => 40,
        NamedColor::Red => 41,
        NamedColor::Green => 42,
        NamedColor::Yellow => 43,
        NamedColor::Blue => 44,
        NamedColor::Magenta => 45,
        NamedColor::Cyan => 46,
        NamedColor::White => 47,
        NamedColor::BrightBlack => 100,
        NamedColor::BrightRed => 101,
        NamedColor::BrightGreen => 102,
        NamedColor::BrightYellow => 103,
        NamedColor::BrightBlue => 104,
        NamedColor::BrightMagenta => 105,
        NamedColor::BrightCyan => 106,
        NamedColor::BrightWhite => 107,
    }
}
