pub mod builtin;
pub mod color;
pub mod syntax;
pub mod validate;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::config::{TuiConfig, TuiThemeSetting, config_path};
use crate::error::{Result, SnipError};

pub use color::{NamedColor, ThemeColor};

pub const THEME_SCHEMA_VERSION: u32 = 1;
const MAX_EXTENDS_DEPTH: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Dark,
    Light,
}

impl Appearance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Theme {
    pub schema_version: u32,
    pub name: String,
    pub display_name: String,
    pub appearance: Appearance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub ui: ThemeUi,
    pub syntax: Syntax,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<Palette>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThemeUi {
    pub background: ThemeColor,
    pub foreground: ThemeColor,
    pub accent: ThemeColor,
    pub accent_alt: ThemeColor,
    pub border: ThemeColor,
    pub muted: ThemeColor,
    pub selection_bg: ThemeColor,
    pub selection_fg: ThemeColor,
    pub retained_bg: ThemeColor,
    pub pill_primary: ThemeColor,
    pub pill_secondary: ThemeColor,
    pub bar_bg: ThemeColor,
    pub bar_fg: ThemeColor,
    pub tag: ThemeColor,
    pub rule: ThemeColor,
    pub success: ThemeColor,
    pub warning: ThemeColor,
    pub error: ThemeColor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Syntax {
    Theme { theme: String },
    Derive { derive: SyntaxDerive },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyntaxDerive {
    Base16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Palette {
    #[serde(flatten)]
    pub colors: BTreeMap<String, ThemeColor>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ThemeSummary {
    pub name: String,
    pub display_name: String,
    pub appearance: Appearance,
    pub source: Option<String>,
    pub builtin: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    schema_version: u32,
    name: String,
    display_name: Option<String>,
    appearance: Appearance,
    source: Option<String>,
    extends: Option<String>,
    // A theme that extends another may override nothing at all, or only
    // `[syntax]`/`[palette]`, so an absent `[ui]` is not an error here. A
    // standalone theme still has to name every role: `require_all_ui` below
    // rejects the empty table when there is no parent to inherit from.
    #[serde(default)]
    ui: RawUi,
    syntax: Option<RawSyntax>,
    palette: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawUi {
    background: Option<String>,
    foreground: Option<String>,
    accent: Option<String>,
    accent_alt: Option<String>,
    border: Option<String>,
    muted: Option<String>,
    selection_bg: Option<String>,
    selection_fg: Option<String>,
    retained_bg: Option<String>,
    pill_primary: Option<String>,
    pill_secondary: Option<String>,
    bar_bg: Option<String>,
    bar_fg: Option<String>,
    tag: Option<String>,
    rule: Option<String>,
    success: Option<String>,
    warning: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSyntax {
    theme: Option<String>,
    derive: Option<SyntaxDerive>,
}

pub fn load(name: &str) -> Result<Theme> {
    let mut visited = HashSet::new();
    load_inner(name, 0, &mut visited)
}

fn load_inner(name: &str, depth: usize, visited: &mut HashSet<String>) -> Result<Theme> {
    if depth > MAX_EXTENDS_DEPTH {
        return Err(SnipError::validation(format!(
            "theme {name}: extends chain too deep"
        )));
    }
    if !visited.insert(name.to_owned()) {
        return Err(SnipError::validation(format!(
            "theme {name}: extends cycle via {name}"
        )));
    }
    let result = (|| {
        let (text, expected) = read_source(name)?;
        let raw = parse_raw(&text, expected.as_deref())?;
        let parent = raw
            .extends
            .as_deref()
            .map(|parent| load_inner(parent, depth + 1, visited))
            .transpose()?;
        resolve_raw(raw, parent)
    })();
    visited.remove(name);
    result
}

fn read_source(name: &str) -> Result<(String, Option<String>)> {
    let path = themes_dir()?.join(format!("{name}.toml"));
    if path.is_file() {
        let text = fs::read_to_string(&path).map_err(|error| {
            SnipError::io(format!("cannot read theme {}: {error}", path.display()))
        })?;
        return Ok((text, Some(name.to_owned())));
    }
    if let Some((_, text)) = builtin::THEMES
        .iter()
        .find(|(builtin_name, _)| *builtin_name == name)
    {
        return Ok(((*text).to_owned(), Some(name.to_owned())));
    }
    Err(SnipError::validation(format!("unknown theme {name}")))
}

fn parse_raw(text: &str, expected_name: Option<&str>) -> Result<RawTheme> {
    let raw: RawTheme = toml::from_str(text)
        .map_err(|error| SnipError::validation(format!("cannot parse theme: {error}")))?;
    validate_theme_name(&raw.name)?;
    if expected_name.is_some_and(|expected| expected != raw.name) {
        return Err(SnipError::validation(format!(
            "theme name {} must equal file stem {}",
            raw.name,
            expected_name.unwrap()
        )));
    }
    if raw.schema_version > THEME_SCHEMA_VERSION {
        return Err(SnipError::validation(format!(
            "theme {} uses schema version {}, but this snip supports up to {}",
            raw.name, raw.schema_version, THEME_SCHEMA_VERSION
        )));
    }
    if raw.schema_version == 0 {
        return Err(SnipError::validation(format!(
            "theme {} has invalid schema version 0",
            raw.name
        )));
    }
    Ok(raw)
}

fn resolve_raw(raw: RawTheme, parent: Option<Theme>) -> Result<Theme> {
    let name = raw.name.clone();
    let mut ui = parent.as_ref().map(|theme| theme.ui.clone());
    if ui.is_none() {
        ui = Some(empty_ui());
    }
    let mut ui = ui.unwrap();
    apply_ui(&mut ui, &raw.ui, &name)?;
    if raw.extends.is_none() {
        require_all_ui(&raw.ui, &name)?;
    }

    let palette = match raw.palette {
        Some(values) => Some(parse_palette(&name, values)?),
        None => parent.as_ref().and_then(|theme| theme.palette.clone()),
    };
    let syntax = match raw.syntax {
        Some(raw) => parse_syntax(&name, raw)?,
        None => parent
            .as_ref()
            .map(|theme| theme.syntax.clone())
            .or_else(|| {
                palette.as_ref().map(|_| Syntax::Derive {
                    derive: SyntaxDerive::Base16,
                })
            })
            .ok_or_else(|| SnipError::validation(format!("theme {name}: missing [syntax]")))?,
    };
    if matches!(syntax, Syntax::Derive { .. }) && palette.is_none() {
        return Err(SnipError::validation(format!(
            "theme {name}: syntax derive = \"base16\" requires [palette]"
        )));
    }
    let theme = Theme {
        schema_version: raw.schema_version,
        display_name: raw.display_name.unwrap_or_else(|| name.clone()),
        name,
        appearance: raw.appearance,
        source: raw
            .source
            .or_else(|| parent.as_ref().and_then(|theme| theme.source.clone())),
        ui,
        syntax,
        palette,
    };
    if let Some(failure) = validate::check(&theme)
        .into_iter()
        .find(|check| check.id == "surface-pairing" && check.level == validate::Level::Fail)
    {
        return Err(SnipError::validation(format!(
            "theme {}: {}",
            theme.name, failure.detail
        )));
    }
    Ok(theme)
}

fn parse_syntax(name: &str, raw: RawSyntax) -> Result<Syntax> {
    match (raw.theme, raw.derive) {
        (Some(theme), None) => {
            syntax::validate_embedded_name(&theme)?;
            Ok(Syntax::Theme { theme })
        }
        (None, Some(derive)) => Ok(Syntax::Derive { derive }),
        _ => Err(SnipError::validation(format!(
            "theme {name}: [syntax] must contain exactly one of theme or derive"
        ))),
    }
}

fn parse_palette(name: &str, values: BTreeMap<String, String>) -> Result<Palette> {
    let expected = (0..16)
        .map(|index| format!("base{index:02X}"))
        .collect::<Vec<_>>();
    for key in &expected {
        if !values.contains_key(key) {
            return Err(SnipError::validation(format!(
                "theme {name}: missing palette key {key}"
            )));
        }
    }
    if values.len() != 16 {
        return Err(SnipError::validation(format!(
            "theme {name}: [palette] must contain exactly base00 through base0F"
        )));
    }
    let mut colors = BTreeMap::new();
    for (key, value) in values {
        if value.len() != 7 || !value.starts_with('#') {
            return Err(SnipError::validation(format!(
                "theme {name}: palette {key} must be a #rrggbb literal"
            )));
        }
        let color = ThemeColor::parse(&value, &key)?;
        if !matches!(color, ThemeColor::Rgb(..)) {
            return Err(SnipError::validation(format!(
                "theme {name}: palette {key} must be a #rrggbb literal"
            )));
        }
        colors.insert(key, color);
    }
    Ok(Palette { colors })
}

macro_rules! ui_fields {
    ($macro:ident) => {
        $macro!(background);
        $macro!(foreground);
        $macro!(accent);
        $macro!(accent_alt);
        $macro!(border);
        $macro!(muted);
        $macro!(selection_bg);
        $macro!(selection_fg);
        $macro!(retained_bg);
        $macro!(pill_primary);
        $macro!(pill_secondary);
        $macro!(bar_bg);
        $macro!(bar_fg);
        $macro!(tag);
        $macro!(rule);
        $macro!(success);
        $macro!(warning);
        $macro!(error);
    };
}

fn apply_ui(ui: &mut ThemeUi, raw: &RawUi, name: &str) -> Result<()> {
    macro_rules! apply {
        ($field:ident) => {
            if let Some(value) = &raw.$field {
                ui.$field = ThemeColor::parse(value, stringify!($field))
                    .map_err(|error| SnipError::validation(format!("theme {name}: {error}")))?;
            }
        };
    }
    ui_fields!(apply);
    Ok(())
}

fn require_all_ui(raw: &RawUi, name: &str) -> Result<()> {
    macro_rules! require {
        ($field:ident) => {
            if raw.$field.is_none() {
                return Err(SnipError::validation(format!(
                    "theme {name}: missing ui key {}",
                    stringify!($field)
                )));
            }
        };
    }
    ui_fields!(require);
    Ok(())
}

fn empty_ui() -> ThemeUi {
    let black = ThemeColor::Rgb(0, 0, 0);
    ThemeUi {
        background: black,
        foreground: black,
        accent: black,
        accent_alt: black,
        border: black,
        muted: black,
        selection_bg: black,
        selection_fg: black,
        retained_bg: black,
        pill_primary: black,
        pill_secondary: black,
        bar_bg: black,
        bar_fg: black,
        tag: black,
        rule: black,
        success: black,
        warning: black,
        error: black,
    }
}

pub fn validate_theme_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        });
    if valid {
        Ok(())
    } else {
        Err(SnipError::validation(format!("invalid theme name {name}")))
    }
}

pub fn themes_dir() -> Result<PathBuf> {
    Ok(config_path()?
        .parent()
        .expect("config path has a parent")
        .join("themes"))
}

/// Read just the `appearance` of a theme that failed to load, so a broken theme
/// still lists — and shows up in the picker — under the right heading. Unlike
/// [`RawTheme`] this ignores every other field, so it survives a bad color, a
/// missing `[ui]` key, or an `extends` that does not resolve. A theme whose TOML
/// does not parse at all has no appearance to read, and the caller falls back.
fn probe_appearance(text: &str) -> Option<Appearance> {
    #[derive(Deserialize)]
    struct Probe {
        appearance: Appearance,
    }

    toml::from_str::<Probe>(text)
        .ok()
        .map(|probe| probe.appearance)
}

pub fn list() -> Vec<ThemeSummary> {
    let mut entries = HashMap::<String, (bool, Option<PathBuf>)>::new();
    for (name, _) in builtin::THEMES {
        entries.insert((*name).to_owned(), (true, None));
    }
    if let Ok(directory) = themes_dir()
        && let Ok(files) = fs::read_dir(directory)
    {
        for entry in files.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("toml")
                && let Some(name) = path.file_stem().and_then(|stem| stem.to_str())
            {
                entries.insert(name.to_owned(), (false, Some(path)));
            }
        }
    }
    let mut summaries = entries
        .into_iter()
        .map(|(name, (builtin, _))| match load(&name) {
            Ok(theme) => ThemeSummary {
                name: theme.name,
                display_name: theme.display_name,
                appearance: theme.appearance,
                source: theme.source,
                builtin,
                error: None,
            },
            Err(error) => ThemeSummary {
                appearance: read_source(&name)
                    .ok()
                    .and_then(|(text, _)| probe_appearance(&text))
                    .unwrap_or(Appearance::Dark),
                name: name.clone(),
                display_name: name,
                source: None,
                builtin,
                error: Some(error.to_string()),
            },
        })
        .collect::<Vec<_>>();
    summaries
        .sort_by(|left, right| (left.appearance, &left.name).cmp(&(right.appearance, &right.name)));
    summaries
}

pub fn resolve_appearance(setting: TuiThemeSetting, environment: Option<&str>) -> Appearance {
    if let Some(value) = environment {
        if value.eq_ignore_ascii_case("light") {
            return Appearance::Light;
        }
        if value.eq_ignore_ascii_case("dark") {
            return Appearance::Dark;
        }
    }
    match setting {
        TuiThemeSetting::Light => return Appearance::Light,
        TuiThemeSetting::Dark => return Appearance::Dark,
        TuiThemeSetting::Auto => {}
    }
    #[cfg(target_os = "macos")]
    {
        if Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .is_ok_and(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .eq_ignore_ascii_case("dark")
            })
        {
            Appearance::Dark
        } else {
            Appearance::Light
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if std::env::var("GTK_THEME").is_ok_and(|value| value.to_ascii_lowercase().contains("dark"))
        {
            return Appearance::Dark;
        }
        if let Ok(value) = std::env::var("COLORFGBG")
            && let Some(background) = value.rsplit(';').next()
            && let Ok(background) = background.parse::<u8>()
        {
            return if background <= 6 || background == 8 {
                Appearance::Dark
            } else {
                Appearance::Light
            };
        }
        Appearance::Dark
    }
}

pub fn resolve(config: &TuiConfig) -> (Theme, Vec<String>) {
    resolve_with_environment(config, std::env::var("SNIP_TUI_THEME").ok().as_deref())
}

/// The body of [`resolve`], with `SNIP_TUI_THEME` passed in rather than read.
/// Tests cannot mutate the process environment safely, so they call this.
pub fn resolve_with_environment(
    config: &TuiConfig,
    environment: Option<&str>,
) -> (Theme, Vec<String>) {
    let mut warnings = Vec::new();
    if let Some(name) = environment.filter(|value| {
        !value.is_empty()
            && !value.eq_ignore_ascii_case("light")
            && !value.eq_ignore_ascii_case("dark")
    }) {
        match load_runtime(name) {
            Ok((mut theme, theme_warnings)) => {
                warnings.extend(theme_warnings);
                apply_overrides(&mut theme, &config.extra, &mut warnings);
                return (theme, warnings);
            }
            Err(error) => warnings.push(error.to_string()),
        }
    }
    let appearance = resolve_appearance(config.theme, environment);
    let default_name = match appearance {
        Appearance::Light => "light-default",
        Appearance::Dark => "dark-default",
    };
    let configured_name = match appearance {
        Appearance::Light => config.light_theme.as_deref(),
        Appearance::Dark => config.dark_theme.as_deref(),
    }
    .unwrap_or(default_name);
    let mut theme = match load_runtime(configured_name) {
        Ok((theme, theme_warnings)) => {
            warnings.extend(theme_warnings);
            theme
        }
        Err(error) => {
            warnings.push(error.to_string());
            load(default_name).expect("built-in default theme must parse")
        }
    };
    if theme.appearance != appearance {
        warnings.push(format!(
            "theme {} is a {} theme in the {} slot",
            theme.name,
            theme.appearance.as_str(),
            appearance.as_str()
        ));
    }
    apply_overrides(&mut theme, &config.extra, &mut warnings);
    (theme, warnings)
}

fn load_runtime(name: &str) -> Result<(Theme, Vec<String>)> {
    let theme = load(name)?;
    let checks = validate::check(&theme);
    if let Some(failure) = checks
        .iter()
        .find(|check| check.level == validate::Level::Fail)
    {
        return Err(SnipError::validation(format!(
            "theme {name}: {}: {}",
            failure.id, failure.detail
        )));
    }
    // `computed-foreground` is a Note-level finding: it only reports that a
    // pill/bar label already falls back to black/white automatically, an
    // expected condition for built-in themes. Note findings are never surfaced
    // as runtime warnings, so they cannot pin a warning over the TUI bottom
    // bar on startup.
    let warnings = checks
        .into_iter()
        .filter(|check| check.level == validate::Level::Warn)
        .map(|check| format!("theme {name}: {}: {}", check.id, check.detail))
        .collect();
    Ok((theme, warnings))
}

fn apply_overrides(theme: &mut Theme, extra: &toml::Table, warnings: &mut Vec<String>) {
    let Some(colors) = extra.get("colors").and_then(toml::Value::as_table) else {
        return;
    };
    let original_surface = (theme.ui.background, theme.ui.foreground);
    macro_rules! apply {
        ($field:ident) => {
            if let Some(value) = colors.get(stringify!($field)) {
                match value.as_str() {
                    Some(value) => match ThemeColor::parse(value, stringify!($field)) {
                        Ok(color) => theme.ui.$field = color,
                        Err(error) => warnings.push(error.to_string()),
                    },
                    None => warnings.push(format!(
                        "invalid color for {}: expected a string",
                        stringify!($field)
                    )),
                }
            }
        };
    }
    ui_fields!(apply);
    if matches!(theme.ui.background, ThemeColor::Terminal)
        != matches!(theme.ui.foreground, ThemeColor::Terminal)
    {
        warnings.push(format!(
            "theme {}: background and foreground must both be set or both be \"terminal\"",
            theme.name
        ));
        (theme.ui.background, theme.ui.foreground) = original_surface;
    }
}

pub fn parse_file(path: &Path) -> Result<Theme> {
    let name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| SnipError::validation("theme path has no UTF-8 file stem"))?;
    let text = fs::read_to_string(path)?;
    let raw = parse_raw(&text, Some(name))?;
    resolve_raw(raw, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_load(
        name: &str,
        sources: &HashMap<String, String>,
        depth: usize,
        visited: &mut HashSet<String>,
    ) -> Result<Theme> {
        if depth > MAX_EXTENDS_DEPTH {
            return Err(SnipError::validation(format!(
                "theme {name}: extends chain too deep"
            )));
        }
        if !visited.insert(name.to_owned()) {
            return Err(SnipError::validation(format!(
                "theme {name}: extends cycle via {name}"
            )));
        }
        let text = sources
            .get(name)
            .ok_or_else(|| SnipError::validation(format!("unknown theme {name}")))?;
        let raw = parse_raw(text, Some(name))?;
        let parent = raw
            .extends
            .as_deref()
            .map(|parent| test_load(parent, sources, depth + 1, visited))
            .transpose()?;
        visited.remove(name);
        resolve_raw(raw, parent)
    }

    fn child(name: &str, parent: &str) -> String {
        format!(
            "schema_version = 1\nname = \"{name}\"\nappearance = \"dark\"\nextends = \"{parent}\"\n\n[ui]\naccent = \"#010203\"\n"
        )
    }

    #[test]
    fn extends_overlays_only_named_roles() {
        let parent = load("dark-default").unwrap();
        let raw = parse_raw(&child("child", "dark-default"), Some("child")).unwrap();
        let theme = resolve_raw(raw, Some(parent.clone())).unwrap();
        assert_eq!(theme.ui.accent, ThemeColor::Rgb(1, 2, 3));
        assert_eq!(theme.ui.error, parent.ui.error);
        assert_eq!(theme.syntax, parent.syntax);
    }

    #[test]
    fn extends_cycles_and_depth_nine_fail() {
        let mut cycle = HashMap::new();
        cycle.insert("a".to_owned(), child("a", "b"));
        cycle.insert("b".to_owned(), child("b", "a"));
        let error = test_load("a", &cycle, 0, &mut HashSet::new()).unwrap_err();
        assert!(error.to_string().contains("extends cycle"));

        let mut deep = HashMap::new();
        for index in 0..=9 {
            let name = format!("theme-{index}");
            let parent = format!("theme-{}", index + 1);
            deep.insert(name.clone(), child(&name, &parent));
        }
        let error = test_load("theme-0", &deep, 0, &mut HashSet::new()).unwrap_err();
        assert_eq!(error.to_string(), "theme theme-9: extends chain too deep");
    }

    #[test]
    fn standalone_theme_missing_a_ui_key_names_that_key() {
        let text = "schema_version = 1\nname = \"incomplete\"\nappearance = \"dark\"\n\n[ui]\nbackground = \"terminal\"\n\n[syntax]\ntheme = \"Nord\"\n";
        let raw = parse_raw(text, Some("incomplete")).unwrap();
        let error = resolve_raw(raw, None).unwrap_err();
        assert!(error.to_string().contains("missing ui key foreground"));
    }

    #[test]
    fn every_builtin_parses_resolves_syntax_and_has_no_unexpected_findings() {
        let mut unexpected_findings = Vec::new();
        for (name, _) in builtin::THEMES {
            let theme = load(name).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(&theme.name, name);
            syntax::resolve(&theme).unwrap_or_else(|error| panic!("{name}: {error}"));
            let findings = validate::check(&theme)
                .into_iter()
                .filter(|check| {
                    matches!(check.level, validate::Level::Warn | validate::Level::Fail)
                })
                .collect::<Vec<_>>();
            if !findings.is_empty() {
                unexpected_findings.push(format!("{name}: {findings:?}"));
            }
        }
        assert!(
            unexpected_findings.is_empty(),
            "{}",
            unexpected_findings.join("\n")
        );
    }

    #[test]
    fn default_themes_use_terminal_surfaces() {
        for name in ["dark-default", "light-default"] {
            let theme = load(name).unwrap();
            assert_eq!(theme.ui.background, ThemeColor::Terminal);
            assert_eq!(theme.ui.foreground, ThemeColor::Terminal);
        }
    }

    #[test]
    fn a_broken_theme_still_reports_the_appearance_it_asked_for() {
        // Everything that fails after the file has parsed as TOML still knows
        // which slot it belongs to, so a broken light theme is not filed —
        // and offered in the picker — as a dark one.
        for (label, text) in [
            (
                "bad color",
                "schema_version = 1\nname = \"x\"\nappearance = \"light\"\nextends = \"light-github\"\n[ui]\naccent = \"nonsense\"\n",
            ),
            (
                "missing ui key",
                "schema_version = 1\nname = \"x\"\nappearance = \"light\"\n[ui]\nbackground = \"terminal\"\n",
            ),
            (
                "unresolvable parent",
                "schema_version = 1\nname = \"x\"\nappearance = \"light\"\nextends = \"nope\"\n",
            ),
            (
                "name mismatch",
                "schema_version = 1\nname = \"other\"\nappearance = \"light\"\nextends = \"light-github\"\n",
            ),
            (
                "future schema",
                "schema_version = 99\nname = \"x\"\nappearance = \"light\"\nextends = \"light-github\"\n",
            ),
        ] {
            assert_eq!(
                probe_appearance(text),
                Some(Appearance::Light),
                "{label} should still report its appearance"
            );
        }

        // Only a file that is not TOML at all leaves the caller to guess.
        assert_eq!(probe_appearance("not toml {{{"), None);
        assert_eq!(
            probe_appearance("schema_version = 1\nname = \"x\"\n"),
            None,
            "a missing appearance cannot be guessed either"
        );
    }

    #[test]
    fn extends_may_override_nothing_at_all() {
        let text = "schema_version = 1\nname = \"child\"\nappearance = \"dark\"\nextends = \"dark-nord\"\n";
        let parent = load("dark-nord").unwrap();
        let raw = parse_raw(text, Some("child")).unwrap();
        let theme = resolve_raw(raw, Some(parent.clone())).unwrap();
        assert_eq!(theme.ui, parent.ui);
        assert_eq!(theme.syntax, parent.syntax);
        assert_eq!(theme.palette, parent.palette);
    }

    #[test]
    fn config_colors_override_the_theme_file() {
        let mut config = TuiConfig {
            theme: TuiThemeSetting::Dark,
            dark_theme: Some("dark-nord".to_owned()),
            ..TuiConfig::default()
        };
        config.extra.insert(
            "colors".to_owned(),
            toml::Value::Table(toml::Table::from_iter([
                ("accent".to_owned(), toml::Value::String("#010203".into())),
                ("bogus".to_owned(), toml::Value::String("#040506".into())),
                ("tag".to_owned(), toml::Value::String("nonsense".into())),
            ])),
        );

        let (theme, warnings) = resolve_with_environment(&config, None);
        let plain = load("dark-nord").unwrap();
        assert_eq!(theme.ui.accent, ThemeColor::Rgb(1, 2, 3));
        assert_eq!(theme.ui.tag, plain.ui.tag, "an invalid override is skipped");
        assert!(warnings.iter().any(|warning| warning.contains("nonsense")));
    }

    #[test]
    fn environment_naming_a_theme_wins_and_an_unknown_one_warns() {
        let config = TuiConfig {
            theme: TuiThemeSetting::Dark,
            dark_theme: Some("dark-nord".to_owned()),
            ..TuiConfig::default()
        };

        let (theme, warnings) = resolve_with_environment(&config, Some("light-gruvbox"));
        assert_eq!(theme.name, "light-gruvbox");
        // Precondition: light-gruvbox still carries the `computed-foreground`
        // finding as a Note, so the "not a runtime warning" assertion below
        // fails loudly (rather than silently passing) if the theme changes.
        let note = validate::check(&load("light-gruvbox").unwrap())
            .into_iter()
            .find(|check| check.id == "computed-foreground")
            .expect("light-gruvbox carries the computed-foreground finding");
        assert_eq!(note.level, validate::Level::Note);
        // `computed-foreground` is an automatic black/white fallback, not a
        // runtime warning, so it is never surfaced to the user.
        assert!(
            warnings
                .iter()
                .all(|warning| !warning.contains("computed-foreground"))
        );

        // "light"/"dark" stay appearance overrides rather than theme names.
        let (theme, _) = resolve_with_environment(&config, Some("light"));
        assert_eq!(theme.name, "light-default");

        let (theme, warnings) = resolve_with_environment(&config, Some("does-not-exist"));
        assert_eq!(theme.name, "dark-nord", "an unknown name falls through");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("unknown theme"))
        );
    }

    #[test]
    fn resolve_unknown_theme_falls_back_with_warning() {
        let config = TuiConfig {
            theme: TuiThemeSetting::Dark,
            dark_theme: Some("does-not-exist".to_owned()),
            ..TuiConfig::default()
        };
        let (theme, warnings) = resolve(&config);
        assert_eq!(theme.name, "dark-default");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("unknown theme"))
        );
    }
}
