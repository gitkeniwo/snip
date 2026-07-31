use std::str::FromStr;
use std::sync::OnceLock;

use syntect::highlighting::{
    Color, ScopeSelectors, StyleModifier, Theme as SyntectTheme, ThemeItem, ThemeSettings,
};
use two_face::theme::{EmbeddedLazyThemeSet, EmbeddedThemeName};

use crate::error::{Result, SnipError};

use super::{Palette, Syntax, Theme, ThemeColor};

static SYNTAX_THEMES: OnceLock<EmbeddedLazyThemeSet> = OnceLock::new();

fn themes() -> &'static EmbeddedLazyThemeSet {
    SYNTAX_THEMES.get_or_init(two_face::theme::extra)
}

fn embedded_name(name: &str) -> Option<EmbeddedThemeName> {
    EmbeddedLazyThemeSet::theme_names()
        .iter()
        .copied()
        .find(|candidate| {
            candidate.as_name().eq_ignore_ascii_case(name)
                || format!("{candidate:?}").eq_ignore_ascii_case(name)
        })
}

pub fn validate_embedded_name(name: &str) -> Result<()> {
    embedded_name(name)
        .map(|_| ())
        .ok_or_else(|| SnipError::validation(format!("unknown syntax theme {name}")))
}

pub fn resolve(theme: &Theme) -> Result<SyntectTheme> {
    match &theme.syntax {
        Syntax::Theme { theme: name } => embedded_name(name)
            .map(|name| themes().get(name).clone())
            .ok_or_else(|| SnipError::validation(format!("unknown syntax theme {name}"))),
        Syntax::Derive { .. } => derive(theme),
    }
}

fn derive(theme: &Theme) -> Result<SyntectTheme> {
    let palette = theme.palette.as_ref().ok_or_else(|| {
        SnipError::validation(format!(
            "theme {}: base16 syntax requires [palette]",
            theme.name
        ))
    })?;
    let mut settings = ThemeSettings {
        foreground: Some(palette_color(palette, "base05")?),
        background: Some(palette_color(palette, "base00")?),
        caret: Some(palette_color(palette, "base05")?),
        selection: Some(palette_color(palette, "base02")?),
        gutter_foreground: Some(palette_color(palette, "base03")?),
        line_highlight: Some(palette_color(palette, "base01")?),
        ..ThemeSettings::default()
    };
    settings.gutter = Some(palette_color(palette, "base00")?);
    let rules = [
        ("comment", "base03"),
        ("string, constant.character", "base0B"),
        ("constant.numeric, constant.language", "base09"),
        ("variable, entity.name.tag", "base08"),
        ("keyword, storage, keyword.operator", "base0E"),
        (
            "entity.name.function, support.function, markup.heading",
            "base0D",
        ),
        (
            "entity.name.class, entity.name.type, support.type, support.class",
            "base0A",
        ),
        (
            "support.constant, constant.character.escape, string.regexp",
            "base0C",
        ),
        ("entity.other.attribute-name", "base09"),
        ("invalid", "base08"),
    ];
    let scopes = rules
        .into_iter()
        .map(|(selector, slot)| {
            let scope = ScopeSelectors::from_str(selector).map_err(|error| {
                SnipError::validation(format!(
                    "invalid built-in scope selector {selector}: {error}"
                ))
            })?;
            Ok(ThemeItem {
                scope,
                style: StyleModifier {
                    foreground: Some(palette_color(palette, slot)?),
                    background: None,
                    font_style: None,
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SyntectTheme {
        name: Some(theme.display_name.clone()),
        author: theme.source.clone(),
        settings,
        scopes,
    })
}

fn palette_color(palette: &Palette, key: &str) -> Result<Color> {
    let color = palette
        .colors
        .get(key)
        .copied()
        .ok_or_else(|| SnipError::validation(format!("missing palette key {key}")))?;
    let ThemeColor::Rgb(r, g, b) = color else {
        return Err(SnipError::validation(format!(
            "palette key {key} is not an RGB color"
        )));
    };
    Ok(Color { r, g, b, a: 255 })
}
