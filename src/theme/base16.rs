use std::collections::{BTreeMap, HashSet};

use crate::error::{Result, SnipError};

use super::color::{contrast, relative_luminance};
use super::syntax;
use super::{
    Appearance, Palette, Syntax, SyntaxDerive, THEME_SCHEMA_VERSION, Theme, ThemeColor, ThemeUi,
    ensure_contrast, validate_theme_name,
};

/// The 16 palette slots of a base16 scheme, in order.
pub const SLOTS: [&str; 16] = [
    "base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07", "base08",
    "base09", "base0A", "base0B", "base0C", "base0D", "base0E", "base0F",
];

/// A parsed base16 scheme: the 16 palette slots plus the metadata the
/// conversion needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scheme {
    pub name: String,
    pub variant: Option<Appearance>,
    pub palette: BTreeMap<String, ThemeColor>,
}

impl Scheme {
    fn slot(&self, key: &str) -> ThemeColor {
        *self
            .palette
            .get(key)
            .expect("parse_scheme guarantees a complete palette")
    }
}

/// Parse the flat base16 YAML subset documented in `docs/themes.md`.
pub fn parse_scheme(text: &str) -> Result<Scheme> {
    let mut name = None;
    let mut variant = None;
    let mut palette_values = BTreeMap::new();
    let mut top_keys = HashSet::new();
    let mut palette_keys = HashSet::new();
    let mut palette_open = false;
    let mut saw_palette = false;

    for (index, raw_line) in text.split('\n').enumerate() {
        let line_number = index + 1;
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indentation = line
            .chars()
            .take_while(|character| character.is_whitespace())
            .collect::<String>();
        if indentation.contains('\t') {
            return Err(SnipError::validation(format!(
                "scheme: tab indentation is not supported on line {line_number}"
            )));
        }
        let indented = !indentation.is_empty();
        if !indented && trimmed == "---" {
            palette_open = false;
            continue;
        }
        if !indented {
            palette_open = false;
        }
        if indented && !palette_open {
            continue;
        }
        let Some((key, value)) = (if indented { line.trim_start() } else { line }).split_once(':')
        else {
            return Err(SnipError::validation(format!(
                "scheme: cannot parse line {line_number}: {line}"
            )));
        };
        let key = key.trim();
        let value = unquote(value.trim());
        if indented {
            if !palette_keys.insert(key.to_owned()) {
                return Err(SnipError::validation(format!(
                    "scheme: duplicate key {key} on line {line_number}"
                )));
            }
            if SLOTS.contains(&key) {
                let color = ThemeColor::parse(value, key).map_err(|_| {
                    SnipError::validation(format!(
                        "scheme: palette {key} must be a #rrggbb literal"
                    ))
                })?;
                if !matches!(color, ThemeColor::Rgb(..)) {
                    return Err(SnipError::validation(format!(
                        "scheme: palette {key} must be a #rrggbb literal"
                    )));
                }
                palette_values.insert(key.to_owned(), color);
            }
            continue;
        }

        if !top_keys.insert(key.to_owned()) {
            return Err(SnipError::validation(format!(
                "scheme: duplicate key {key} on line {line_number}"
            )));
        }
        match key {
            "name" => name = Some(value.to_owned()),
            "variant" => {
                variant = Some(match value.to_ascii_lowercase().as_str() {
                    "dark" => Appearance::Dark,
                    "light" => Appearance::Light,
                    _ => {
                        return Err(SnipError::validation(format!(
                            "scheme: invalid variant {value}"
                        )));
                    }
                });
            }
            "palette" => {
                if !value.is_empty() {
                    return Err(SnipError::validation(format!(
                        "scheme: cannot parse line {line_number}: {line}"
                    )));
                }
                palette_open = true;
                saw_palette = true;
            }
            _ => {}
        }
    }
    let name = name
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SnipError::validation("scheme: missing name"))?;
    if !saw_palette {
        return Err(SnipError::validation("scheme: missing palette key base00"));
    }
    for key in SLOTS {
        if !palette_values.contains_key(key) {
            return Err(SnipError::validation(format!(
                "scheme: missing palette key {key}"
            )));
        }
    }
    Ok(Scheme {
        name,
        variant,
        palette: palette_values,
    })
}

fn unquote(value: &str) -> &str {
    if value.len() >= 2 {
        let first = value.as_bytes()[0];
        let last = value.as_bytes()[value.len() - 1];
        if matches!((first, last), (b'\'', b'\'') | (b'"', b'"')) {
            return &value[1..value.len() - 1];
        }
        // Upstream scheme files occasionally add a YAML comment after a
        // quoted value. The quoted scalar still has an unambiguous boundary;
        // accepting it keeps the parser compatible with that flat subset
        // without interpreting comments in unquoted values.
        if matches!(first, b'\'' | b'"')
            && let Some(end) = value[1..].find(first as char)
        {
            return &value[1..end + 1];
        }
    }
    value
}

/// Convert a scheme into a complete standalone theme.
pub fn scheme_to_theme(
    scheme: &Scheme,
    name: &str,
    source: &str,
    syntax_theme: Option<&str>,
) -> Result<Theme> {
    validate_theme_name(name)?;
    let selection_bg = scheme.slot("base02");
    let candidates = ["base00", "base05", "base06", "base07"];
    let mut selection_fg = scheme.slot(candidates[0]);
    let mut selection_contrast =
        contrast(selection_fg, selection_bg).expect("scheme colors are RGB");
    for candidate in candidates.into_iter().skip(1) {
        let color = scheme.slot(candidate);
        let value = contrast(color, selection_bg).expect("scheme colors are RGB");
        if value > selection_contrast {
            selection_fg = color;
            selection_contrast = value;
        }
    }
    if selection_contrast < 4.5 {
        return Err(SnipError::validation(format!(
            "theme {name}: selection contrast {selection_contrast:.2} < 4.5"
        )));
    }
    let background = scheme.slot("base00");
    let bar_background = scheme.slot("base01");
    let appearance = scheme.variant.unwrap_or_else(|| {
        if relative_luminance(background).expect("scheme colors are RGB") < 0.5 {
            Appearance::Dark
        } else {
            Appearance::Light
        }
    });
    let slot = |key| scheme.slot(key);
    let ui = ThemeUi {
        background,
        foreground: slot("base05"),
        accent: ensure_contrast(slot("base0D"), background, 4.5)?,
        accent_alt: ensure_contrast(slot("base0E"), background, 4.5)?,
        border: ensure_contrast(slot("base03"), background, 2.5)?,
        muted: ensure_contrast(slot("base04"), background, 4.5)?,
        selection_bg,
        selection_fg,
        retained_bg: slot("base01"),
        pill_primary: slot("base0C"),
        pill_secondary: slot("base03"),
        bar_bg: bar_background,
        bar_fg: ensure_contrast(slot("base05"), bar_background, 4.5)?,
        tag: ensure_contrast(slot("base09"), background, 4.5)?,
        rule: ensure_contrast(slot("base02"), background, 3.0)?,
        success: ensure_contrast(slot("base0B"), background, 4.5)?,
        warning: ensure_contrast(slot("base0A"), background, 4.5)?,
        error: ensure_contrast(slot("base08"), background, 4.5)?,
    };
    let syntax = match syntax_theme {
        Some(theme) => {
            syntax::validate_embedded_name(theme)?;
            Syntax::Theme {
                theme: theme.to_owned(),
            }
        }
        None => Syntax::Derive {
            derive: SyntaxDerive::Base16,
        },
    };
    Ok(Theme {
        schema_version: THEME_SCHEMA_VERSION,
        name: name.to_owned(),
        display_name: scheme.name.clone(),
        appearance,
        source: Some(source.to_owned()),
        ui,
        syntax,
        palette: Some(Palette {
            colors: scheme.palette.clone(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const NORD: &str = include_str!("../../assets/base16/nord.yaml");

    #[test]
    fn parses_nord_and_base24_variants() {
        let scheme = parse_scheme(NORD).unwrap();
        assert_eq!(scheme.name, "Nord");
        assert_eq!(scheme.variant, Some(Appearance::Dark));
        assert_eq!(scheme.palette.len(), 16);
        let base24 = format!("{NORD}\n  base10: #010101\n  base17: #020202\n");
        assert_eq!(parse_scheme(&base24).unwrap().palette.len(), 16);
    }

    #[test]
    fn parser_accepts_flat_conveniences_and_rejects_invalid_data() {
        let text = "system: base16\nauthor: Someone\nname: 'Nord'\nvariant: DARK\npalette:\n  # comment\n\n  base00: #2e3440\n  base01: '#3b4252'\n  base02: #434c5e\n  base03: #4c566a\n  base04: #d8dee9\n  base05: #e5e9f0\n  base06: #eceff4\n  base07: #8fbcbb\n  base08: #bf616a\n  base09: #d08770\n  base0A: #ebcb8b\n  base0B: #a3be8c\n  base0C: #88c0d0\n  base0D: #81a1c1\n  base0E: #b48ead\n  base0F: #5e81ac\n";
        assert_eq!(parse_scheme(text).unwrap().name, "Nord");
        assert_eq!(parse_scheme(&format!("---\n{text}")).unwrap().name, "Nord");
        assert!(
            parse_scheme(&text.replace("base0A", "base0a"))
                .unwrap_err()
                .to_string()
                .contains("base0A")
        );
        assert!(
            parse_scheme(&text.replace("  base00", "\tbase00"))
                .unwrap_err()
                .to_string()
                .contains("line 8")
        );
        assert!(
            parse_scheme(&format!("{text}  base00: #000000\n"))
                .unwrap_err()
                .to_string()
                .contains("duplicate key base00")
        );
        assert!(
            parse_scheme(&text.replace("variant: DARK", "variant: sideways"))
                .unwrap_err()
                .to_string()
                .contains("invalid variant")
        );
    }

    #[test]
    fn conversion_round_trips_and_checks_validation_inputs() {
        let scheme = parse_scheme(NORD).unwrap();
        let theme = scheme_to_theme(&scheme, "nord-copy", "base16:nord", None).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nord-copy.toml");
        std::fs::write(&path, super::super::to_toml(&theme).unwrap()).unwrap();
        assert_eq!(super::super::parse_file(&path).unwrap(), theme);
        assert!(scheme_to_theme(&scheme, "nord-copy", "base16:nord", Some("NotATheme")).is_err());
    }

    #[test]
    fn conversion_rejects_low_selection_contrast() {
        let scheme = parse_scheme("name: Grey\npalette:\n  base00: #777777\n  base01: #777777\n  base02: #777777\n  base03: #777777\n  base04: #777777\n  base05: #777777\n  base06: #777777\n  base07: #777777\n  base08: #777777\n  base09: #777777\n  base0A: #777777\n  base0B: #777777\n  base0C: #777777\n  base0D: #777777\n  base0E: #777777\n  base0F: #777777\n").unwrap();
        assert!(
            scheme_to_theme(&scheme, "grey", "base16:grey", None)
                .unwrap_err()
                .to_string()
                .contains("selection contrast")
        );
    }
}
