#[cfg(target_os = "macos")]
use std::process::Command;

use ratatui::style::{Color, Modifier, Style};

use crate::config::TuiThemeSetting;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug)]
pub struct TuiTheme {
    pub appearance: Appearance,
    pub accent: Color,
    pub accent_alt: Color,
    pub border: Color,
    pub muted: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub retained_bg: Color,
    pub pill_primary: Color,
    pub pill_secondary: Color,
    pub bar_bg: Color,
    pub bar_fg: Color,
    pub tag: Color,
    pub rule: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
}

impl TuiTheme {
    pub fn detect() -> Self {
        Self::resolve(TuiThemeSetting::Auto)
    }

    pub fn resolve(setting: TuiThemeSetting) -> Self {
        let environment = std::env::var("SNIP_TUI_THEME").ok();
        Self::for_appearance(resolve_appearance(setting, environment.as_deref()))
    }

    /// Reserved for `[tui.colors]`; v2 deliberately keeps the built-in palette.
    pub fn with_overrides(self, _overrides: &toml::Table) -> Self {
        self
    }

    pub fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self {
                appearance,
                accent: Color::Rgb(0, 95, 115),
                accent_alt: Color::Rgb(111, 45, 168),
                border: Color::Rgb(105, 105, 105),
                muted: Color::Rgb(92, 99, 108),
                selection_bg: Color::Rgb(0, 95, 115),
                selection_fg: Color::White,
                retained_bg: Color::Rgb(226, 238, 241),
                pill_primary: Color::Rgb(0, 95, 115),
                pill_secondary: Color::Rgb(211, 219, 224),
                bar_bg: Color::Rgb(225, 228, 232),
                bar_fg: Color::Rgb(42, 47, 52),
                tag: Color::Rgb(154, 100, 0),
                rule: Color::Rgb(210, 214, 219),
                success: Color::Rgb(0, 120, 70),
                warning: Color::Rgb(174, 91, 0),
                error: Color::Rgb(190, 38, 38),
            },
            Appearance::Dark => Self {
                appearance,
                accent: Color::Rgb(88, 166, 255),
                accent_alt: Color::Rgb(210, 168, 255),
                border: Color::Rgb(110, 118, 129),
                muted: Color::Rgb(139, 148, 158),
                selection_bg: Color::Rgb(31, 82, 128),
                selection_fg: Color::White,
                retained_bg: Color::Rgb(28, 39, 50),
                pill_primary: Color::Rgb(0, 113, 118),
                pill_secondary: Color::Rgb(52, 59, 66),
                bar_bg: Color::Rgb(36, 41, 47),
                bar_fg: Color::Rgb(218, 223, 228),
                tag: Color::Rgb(227, 179, 65),
                rule: Color::Rgb(60, 66, 74),
                success: Color::Rgb(63, 185, 80),
                warning: Color::Rgb(227, 179, 65),
                error: Color::Rgb(248, 81, 73),
            },
        }
    }

    pub fn selected(self) -> Style {
        Style::default()
            .fg(self.selection_fg)
            .bg(self.selection_bg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn retained_selection(self) -> Style {
        Style::default()
            .fg(self.accent)
            .bg(self.retained_bg)
            .add_modifier(Modifier::BOLD)
    }
}

fn resolve_appearance(setting: TuiThemeSetting, environment: Option<&str>) -> Appearance {
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
        let is_dark = Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .is_ok_and(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .eq_ignore_ascii_case("dark")
            });
        if is_dark {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_palettes_have_distinct_selection_colors() {
        let light = TuiTheme::for_appearance(Appearance::Light);
        let dark = TuiTheme::for_appearance(Appearance::Dark);
        assert_ne!(light.selection_bg, dark.selection_bg);
        assert_ne!(light.retained_bg, dark.retained_bg);
        assert_ne!(light.pill_primary, dark.pill_primary);
        assert_ne!(light.pill_secondary, dark.pill_secondary);
        assert_ne!(light.accent_alt, dark.accent_alt);
        assert_ne!(light.bar_bg, dark.bar_bg);
        assert_ne!(light.bar_fg, dark.bar_fg);
        assert_ne!(light.tag, dark.tag);
        assert_ne!(light.rule, dark.rule);
    }

    #[test]
    fn environment_override_precedes_explicit_theme() {
        assert_eq!(
            resolve_appearance(TuiThemeSetting::Dark, Some("light")),
            Appearance::Light
        );
        assert_eq!(
            resolve_appearance(TuiThemeSetting::Light, Some("dark")),
            Appearance::Dark
        );
        assert_eq!(
            resolve_appearance(TuiThemeSetting::Dark, None),
            Appearance::Dark
        );
    }
}
