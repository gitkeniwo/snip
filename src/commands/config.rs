use clap::CommandFactory;
use serde_json::json;
use snip::Library;
use snip::config::{
    AppConfig, ColorSetting, OutputSetting, PreviewRenderSetting, TuiConfig, TuiIconSetting,
    TuiThemeSetting, config_path,
};
use snip::error::{Result, SnipError};
use snip::sort::SortMode;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use super::output::{print_record, resolve_output};
use super::system::run_process;
use crate::cli::{
    Cli, CompletionArgs, CompletionShell, ConfigArgs, ConfigCommand, ConfigKey, InitArgs,
    OutputMode,
};

pub fn command_init(args: &InitArgs, output: OutputMode) -> Result<()> {
    let library = Library::init(&args.path, args.name.as_deref())?;
    if args.git {
        run_process(
            ProcessCommand::new("git")
                .arg("init")
                .current_dir(library.root()),
            "git init",
        )?;
    }
    let value = json!({
        "path": library.root(),
        "id": library.manifest().id,
        "name": library.manifest().name,
        "schema_version": library.manifest().schema_version,
        "git_initialized": args.git,
    });
    if output == OutputMode::Human {
        println!("initialized: {}", library.root().display());
        println!("library id: {}", library.manifest().id);
    } else {
        print_record(&value, output)?;
    }
    Ok(())
}

pub fn command_config(args: &ConfigArgs, explicit_output: Option<OutputMode>) -> Result<()> {
    let path = config_path()?;
    match &args.command {
        ConfigCommand::Path => {
            println!("{}", path.display());
            Ok(())
        }
        ConfigCommand::Init { library, force } => {
            if path.exists() && !force {
                return Err(SnipError::conflict(format!(
                    "config already exists: {}; pass --force to replace it",
                    path.display()
                )));
            }
            let mut config = AppConfig {
                output: Some(OutputSetting::Human),
                color: Some(ColorSetting::Auto),
                preview_render: Some(PreviewRenderSetting::Ansi),
                preview_pager: Some(false),
                default_language: Some("text".to_owned()),
                tui: Some(TuiConfig::default()),
                ..AppConfig::default()
            };
            if let Some(library) = library {
                config.default_library = Some(validated_library_path(library)?);
            }
            config.save_to(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
        ConfigCommand::Show => {
            let config = AppConfig::load_from(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
        ConfigCommand::Set { key, value } => {
            let mut config = AppConfig::load_from(&path)?;
            set_config_value(&mut config, *key, value)?;
            config.save_to(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
        ConfigCommand::Unset { key } => {
            let mut config = AppConfig::load_from(&path)?;
            unset_config_value(&mut config, *key);
            config.save_to(&path)?;
            let output = resolve_output(explicit_output, &config);
            print_config(&config, &path, output)
        }
    }
}

pub fn command_completion(args: &CompletionArgs) -> Result<()> {
    let mut command = Cli::command();
    let shell = match args.shell {
        CompletionShell::Bash => clap_complete::Shell::Bash,
        CompletionShell::Elvish => clap_complete::Shell::Elvish,
        CompletionShell::Fish => clap_complete::Shell::Fish,
        CompletionShell::Powershell => clap_complete::Shell::PowerShell,
        CompletionShell::Zsh => clap_complete::Shell::Zsh,
    };
    clap_complete::generate(shell, &mut command, "snip", &mut io::stdout());
    Ok(())
}

fn print_config(config: &AppConfig, path: &Path, output: OutputMode) -> Result<()> {
    if output == OutputMode::Human {
        println!("config: {}", path.display());
        print!("{}", toml::to_string_pretty(config)?);
        Ok(())
    } else {
        print_record(&json!({"path": path, "config": config}), output)
    }
}

fn set_config_value(config: &mut AppConfig, key: ConfigKey, value: &str) -> Result<()> {
    match key {
        ConfigKey::DefaultLibrary => {
            config.default_library = Some(validated_library_path(&expand_user_path(value)?)?);
        }
        ConfigKey::Output => {
            config.output = Some(match value.to_ascii_lowercase().as_str() {
                "human" => OutputSetting::Human,
                "json" => OutputSetting::Json,
                "jsonl" => OutputSetting::Jsonl,
                _ => return Err(SnipError::usage("output must be human, json, or jsonl")),
            });
        }
        ConfigKey::Color => {
            config.color = Some(match value.to_ascii_lowercase().as_str() {
                "auto" => ColorSetting::Auto,
                "always" => ColorSetting::Always,
                "never" => ColorSetting::Never,
                _ => return Err(SnipError::usage("color must be auto, always, or never")),
            });
        }
        ConfigKey::PreviewRender => {
            config.preview_render = Some(match value.to_ascii_lowercase().as_str() {
                "ansi" => PreviewRenderSetting::Ansi,
                "plain" => PreviewRenderSetting::Plain,
                "html" => PreviewRenderSetting::Html,
                _ => {
                    return Err(SnipError::usage(
                        "preview-render must be ansi, plain, or html",
                    ));
                }
            });
        }
        ConfigKey::PreviewPager => config.preview_pager = Some(parse_bool(value)?),
        ConfigKey::Editor => config.editor = Some(nonempty_value("editor", value)?),
        ConfigKey::Pager => config.pager = Some(nonempty_value("pager", value)?),
        ConfigKey::DefaultLanguage => {
            config.default_language = Some(nonempty_value("default-language", value)?)
        }
        ConfigKey::DefaultFolder => config.default_folder = Some(value.trim().to_owned()),
        ConfigKey::DefaultTags => {
            let tags = value.split(',').map(str::to_owned).collect::<Vec<_>>();
            config.default_tags = snip::filesystem::normalize_tags(&tags)?;
        }
        ConfigKey::TuiTheme => {
            config.tui.get_or_insert_with(TuiConfig::default).theme =
                match value.to_ascii_lowercase().as_str() {
                    "auto" => TuiThemeSetting::Auto,
                    "light" => TuiThemeSetting::Light,
                    "dark" => TuiThemeSetting::Dark,
                    _ => return Err(SnipError::usage("tui-theme must be auto, light, or dark")),
                };
        }
        ConfigKey::TuiSort => {
            config.tui.get_or_insert_with(TuiConfig::default).sort =
                match value.to_ascii_lowercase().as_str() {
                    "manual" => SortMode::Manual,
                    "title" => SortMode::Title,
                    "modified" => SortMode::Modified,
                    "created" => SortMode::Created,
                    _ => {
                        return Err(SnipError::usage(
                            "tui-sort must be manual, title, modified, or created",
                        ));
                    }
                };
        }
        ConfigKey::TuiIcons => {
            config.tui.get_or_insert_with(TuiConfig::default).icons =
                match value.to_ascii_lowercase().as_str() {
                    "ascii" => TuiIconSetting::Ascii,
                    "nerd" => TuiIconSetting::Nerd,
                    _ => return Err(SnipError::usage("tui-icons must be ascii or nerd")),
                };
        }
    }
    Ok(())
}

fn unset_config_value(config: &mut AppConfig, key: ConfigKey) {
    match key {
        ConfigKey::DefaultLibrary => config.default_library = None,
        ConfigKey::Output => config.output = None,
        ConfigKey::Color => config.color = None,
        ConfigKey::PreviewRender => config.preview_render = None,
        ConfigKey::PreviewPager => config.preview_pager = None,
        ConfigKey::Editor => config.editor = None,
        ConfigKey::Pager => config.pager = None,
        ConfigKey::DefaultLanguage => config.default_language = None,
        ConfigKey::DefaultFolder => config.default_folder = None,
        ConfigKey::DefaultTags => config.default_tags.clear(),
        ConfigKey::TuiTheme => {
            config.tui.get_or_insert_with(TuiConfig::default).theme = TuiThemeSetting::Auto
        }
        ConfigKey::TuiSort => {
            config.tui.get_or_insert_with(TuiConfig::default).sort = SortMode::Manual
        }
        ConfigKey::TuiIcons => {
            config.tui.get_or_insert_with(TuiConfig::default).icons = TuiIconSetting::Ascii
        }
    }
}

fn validated_library_path(path: &Path) -> Result<PathBuf> {
    Ok(Library::open(path)?.root().to_path_buf())
}

fn expand_user_path(value: &str) -> Result<PathBuf> {
    if value == "~" || value.starts_with("~/") {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| SnipError::io("cannot expand ~: HOME is not set"))?;
        let mut path = PathBuf::from(home);
        if value.len() > 2 {
            path.push(&value[2..]);
        }
        Ok(path)
    } else {
        Ok(PathBuf::from(value))
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(SnipError::usage(
            "boolean value must be true/false, yes/no, on/off, or 1/0",
        )),
    }
}

fn nonempty_value(name: &str, value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        Err(SnipError::usage(format!("{name} cannot be empty")))
    } else {
        Ok(value.to_owned())
    }
}
