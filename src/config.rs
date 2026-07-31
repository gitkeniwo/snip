use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Result, SnipError};
use crate::filesystem::{atomic_write, normalize_tags};
use crate::sort::SortMode;

pub const CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputSetting {
    Human,
    Json,
    Jsonl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorSetting {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewRenderSetting {
    Ansi,
    Plain,
    Html,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TuiThemeSetting {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TuiDensitySetting {
    #[default]
    Comfortable,
    Compact,
}

impl TuiDensitySetting {
    pub const fn next(self) -> Self {
        match self {
            Self::Comfortable => Self::Compact,
            Self::Compact => Self::Comfortable,
        }
    }

    pub const fn row_height(self) -> u16 {
        match self {
            Self::Comfortable => 2,
            Self::Compact => 1,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Compact => "compact",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TuiConfig {
    #[serde(default)]
    pub theme: TuiThemeSetting,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dark_theme: Option<String>,
    #[serde(default)]
    pub sort: SortMode,
    #[serde(default)]
    pub density: TuiDensitySetting,
    // Line numbers are on unless the user turns them off, so this one field
    // cannot lean on `Default::default()` the way its neighbours do.
    #[serde(default = "default_line_numbers")]
    pub line_numbers: bool,
    #[serde(flatten)]
    pub extra: toml::Table,
}

pub(crate) const fn default_line_numbers() -> bool {
    true
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            theme: TuiThemeSetting::default(),
            light_theme: None,
            dark_theme: None,
            sort: SortMode::default(),
            density: TuiDensitySetting::default(),
            line_numbers: default_line_numbers(),
            extra: toml::Table::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GitConfig {
    #[serde(default, alias = "auto_backup_interval")]
    pub auto_commit_interval: u32,
    #[serde(default)]
    pub auto_push: bool,
    #[serde(default)]
    pub backup_on_quit: bool,
    #[serde(flatten)]
    pub extra: toml::Table,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_library: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_render: Option<PreviewRenderSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_pager: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vscode_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pager: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_folder: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tui: Option<TuiConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<GitConfig>,
    #[serde(flatten)]
    pub extra: toml::Table,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            default_library: None,
            output: None,
            color: None,
            preview_render: None,
            preview_pager: None,
            editor: None,
            vscode_cmd: None,
            pager: None,
            default_language: None,
            default_folder: None,
            default_tags: Vec::new(),
            tui: None,
            git: None,
            extra: toml::Table::new(),
        }
    }
}

impl AppConfig {
    /// Load the user configuration, or return defaults when the file is absent.
    pub fn load() -> Result<Self> {
        Self::load_from(&config_path()?)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path).map_err(|error| {
            SnipError::io(format!("cannot read config {}: {error}", path.display()))
        })?;
        let mut config: Self = toml::from_str(&text).map_err(|error| {
            SnipError::validation(format!("cannot parse config {}: {error}", path.display()))
        })?;
        config.validate(path)?;
        config.default_tags = normalize_tags(&config.default_tags)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&config_path()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        self.validate(path)?;
        let data = toml::to_string_pretty(self)?;
        atomic_write(path, data.as_bytes())
    }

    fn validate(&self, path: &Path) -> Result<()> {
        if self.schema_version > CONFIG_SCHEMA_VERSION {
            return Err(SnipError::validation(format!(
                "config {} uses schema version {}, but this snip supports up to {}",
                path.display(),
                self.schema_version,
                CONFIG_SCHEMA_VERSION
            )));
        }
        if self.schema_version == 0 {
            return Err(SnipError::validation(format!(
                "config {} has invalid schema version 0",
                path.display()
            )));
        }
        for (name, value) in [
            ("editor", self.editor.as_deref()),
            ("vscode_cmd", self.vscode_cmd.as_deref()),
            ("pager", self.pager.as_deref()),
            ("default_language", self.default_language.as_deref()),
            (
                "tui.light_theme",
                self.tui
                    .as_ref()
                    .and_then(|config| config.light_theme.as_deref()),
            ),
            (
                "tui.dark_theme",
                self.tui
                    .as_ref()
                    .and_then(|config| config.dark_theme.as_deref()),
            ),
        ] {
            if value.is_some_and(|value| value.trim().is_empty()) {
                return Err(SnipError::validation(format!(
                    "config value {name} cannot be empty"
                )));
            }
        }
        normalize_tags(&self.default_tags)?;
        Ok(())
    }
}

pub fn config_path() -> Result<PathBuf> {
    if let Some(root) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(root).join("snip/config.toml"));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SnipError::io("cannot locate config: HOME or USERPROFILE is not set"))?;
    Ok(PathBuf::from(home).join(".config/snip/config.toml"))
}
