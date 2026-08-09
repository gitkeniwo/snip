use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Result, SnipError};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ThemeColor {
    Rgb(u8, u8, u8),
    Indexed(u8),
    Named(NamedColor),
    Terminal,
}

impl ThemeColor {
    pub fn parse(value: &str, role: &str) -> Result<Self> {
        if value.eq_ignore_ascii_case("terminal") {
            if matches!(role, "background" | "foreground") {
                return Ok(Self::Terminal);
            }
            return Err(SnipError::validation(format!(
                "color role {role} cannot be \"terminal\""
            )));
        }
        if let Some(hex) = value.strip_prefix('#') {
            let expanded;
            let hex = if hex.len() == 3 {
                expanded = hex.chars().flat_map(|ch| [ch, ch]).collect::<String>();
                expanded.as_str()
            } else {
                hex
            };
            if hex.len() == 6
                && let (Ok(red), Ok(green), Ok(blue)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                )
            {
                return Ok(Self::Rgb(red, green, blue));
            }
        }
        if let Some(index) = value.strip_prefix("ansi:")
            && let Ok(index) = index.parse::<u8>()
        {
            return Ok(Self::Indexed(index));
        }
        if let Some(color) = NamedColor::parse(value) {
            return Ok(Self::Named(color));
        }
        Err(SnipError::validation(format!(
            "invalid color \"{value}\" for {role}: expected #rgb, #rrggbb, ansi:N, an ANSI color name, or \"terminal\""
        )))
    }

    pub fn as_string(self) -> String {
        match self {
            Self::Rgb(red, green, blue) => format!("#{red:02x}{green:02x}{blue:02x}"),
            Self::Indexed(index) => format!("ansi:{index}"),
            Self::Named(color) => color.as_str().to_owned(),
            Self::Terminal => "terminal".to_owned(),
        }
    }
}

impl NamedColor {
    fn parse(value: &str) -> Option<Self> {
        Some(match value.to_ascii_lowercase().as_str() {
            "black" => Self::Black,
            "red" => Self::Red,
            "green" => Self::Green,
            "yellow" => Self::Yellow,
            "blue" => Self::Blue,
            "magenta" => Self::Magenta,
            "cyan" => Self::Cyan,
            "white" => Self::White,
            "bright-black" => Self::BrightBlack,
            "bright-red" => Self::BrightRed,
            "bright-green" => Self::BrightGreen,
            "bright-yellow" => Self::BrightYellow,
            "bright-blue" => Self::BrightBlue,
            "bright-magenta" => Self::BrightMagenta,
            "bright-cyan" => Self::BrightCyan,
            "bright-white" => Self::BrightWhite,
            _ => return None,
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Red => "red",
            Self::Green => "green",
            Self::Yellow => "yellow",
            Self::Blue => "blue",
            Self::Magenta => "magenta",
            Self::Cyan => "cyan",
            Self::White => "white",
            Self::BrightBlack => "bright-black",
            Self::BrightRed => "bright-red",
            Self::BrightGreen => "bright-green",
            Self::BrightYellow => "bright-yellow",
            Self::BrightBlue => "bright-blue",
            Self::BrightMagenta => "bright-magenta",
            Self::BrightCyan => "bright-cyan",
            Self::BrightWhite => "bright-white",
        }
    }
}

impl fmt::Display for ThemeColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_string())
    }
}

impl Serialize for ThemeColor {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.as_string())
    }
}

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        ThemeColor::parse(&value, "color").map_err(serde::de::Error::custom)
    }
}

pub fn relative_luminance(color: ThemeColor) -> Option<f64> {
    let ThemeColor::Rgb(red, green, blue) = color else {
        return None;
    };
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * channel(red) + 0.7152 * channel(green) + 0.0722 * channel(blue))
}

pub fn contrast(left: ThemeColor, right: ThemeColor) -> Option<f64> {
    let left = relative_luminance(left)?;
    let right = relative_luminance(right)?;
    Some((left.max(right) + 0.05) / (left.min(right) + 0.05))
}

/// Blend `from` toward `to`. `ratio` is 0.0 (all `from`) to 1.0 (all `to`).
pub fn mix(from: ThemeColor, to: ThemeColor, ratio: f64) -> Result<ThemeColor> {
    let (
        ThemeColor::Rgb(from_red, from_green, from_blue),
        ThemeColor::Rgb(to_red, to_green, to_blue),
    ) = (from, to)
    else {
        return Err(SnipError::validation(format!(
            "cannot blend {} toward {}: both colors must be #rrggbb",
            from.as_string(),
            to.as_string()
        )));
    };
    if !(0.0..=1.0).contains(&ratio) {
        return Err(SnipError::validation(format!(
            "cannot blend colors with ratio {ratio}: expected 0.0 through 1.0"
        )));
    }
    let step = (ratio * 255.0).round() as u16;
    let blend = |from: u8, to: u8| {
        ((u16::from(from) * (255 - step) + u16::from(to) * step + 127) / 255) as u8
    };
    Ok(ThemeColor::Rgb(
        blend(from_red, to_red),
        blend(from_green, to_green),
        blend(from_blue, to_blue),
    ))
}

/// Blend `color` toward white or black — whichever suits `background` — until
/// it clears `floor`. Returns `color` unchanged when it already does.
pub fn ensure_contrast(
    color: ThemeColor,
    background: ThemeColor,
    floor: f64,
) -> Result<ThemeColor> {
    let (
        ThemeColor::Rgb(red, green, blue),
        ThemeColor::Rgb(background_red, background_green, background_blue),
    ) = (color, background)
    else {
        return Err(SnipError::validation(format!(
            "cannot adjust {} against {}: both colors must be #rrggbb",
            color.as_string(),
            background.as_string()
        )));
    };
    let background = ThemeColor::Rgb(background_red, background_green, background_blue);
    if contrast(color, background).expect("RGB colors have contrast") >= floor {
        return Ok(color);
    }
    let target = if relative_luminance(background).expect("RGB colors have luminance") < 0.5 {
        (255, 255, 255)
    } else {
        (0, 0, 0)
    };
    for step in 1..=255_u16 {
        let blend = |from: u8, to: u8| {
            ((u16::from(from) * (255 - step) + u16::from(to) * step + 127) / 255) as u8
        };
        let candidate = ThemeColor::Rgb(
            blend(red, target.0),
            blend(green, target.1),
            blend(blue, target.2),
        );
        if contrast(candidate, background).expect("RGB colors have contrast") >= floor {
            return Ok(candidate);
        }
    }
    Err(SnipError::validation(format!(
        "cannot adjust {} to contrast {floor:.1}",
        color.as_string()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_color_form_and_boundaries() {
        assert_eq!(
            ThemeColor::parse("#f0a", "accent").unwrap(),
            ThemeColor::Rgb(255, 0, 170)
        );
        assert_eq!(
            ThemeColor::parse("#A0b1C2", "accent").unwrap(),
            ThemeColor::Rgb(160, 177, 194)
        );
        assert_eq!(
            ThemeColor::parse("ansi:0", "accent").unwrap(),
            ThemeColor::Indexed(0)
        );
        assert_eq!(
            ThemeColor::parse("ansi:255", "accent").unwrap(),
            ThemeColor::Indexed(255)
        );
        assert_eq!(
            ThemeColor::parse("Red", "accent").unwrap(),
            ThemeColor::Named(NamedColor::Red)
        );
        assert_eq!(
            ThemeColor::parse("terminal", "background").unwrap(),
            ThemeColor::Terminal
        );
        assert_eq!(
            ThemeColor::parse("TERMINAL", "foreground").unwrap(),
            ThemeColor::Terminal
        );
    }

    #[test]
    fn rejects_invalid_colors_and_terminal_roles() {
        for value in ["ansi:256", "#gggggg", "#abcd", ""] {
            assert!(ThemeColor::parse(value, "accent").is_err(), "{value}");
        }
        assert_eq!(
            ThemeColor::parse("terminal", "accent")
                .unwrap_err()
                .to_string(),
            "color role accent cannot be \"terminal\""
        );
    }

    #[test]
    fn serialization_round_trips_every_variant() {
        for color in [
            ThemeColor::Rgb(1, 2, 3),
            ThemeColor::Indexed(42),
            ThemeColor::Named(NamedColor::BrightCyan),
            ThemeColor::Terminal,
        ] {
            let role = if color == ThemeColor::Terminal {
                "background"
            } else {
                "accent"
            };
            assert_eq!(ThemeColor::parse(&color.as_string(), role).unwrap(), color);
        }
    }

    #[test]
    fn ensure_contrast_returns_or_adjusts_rgb_colors() {
        let white = ThemeColor::Rgb(255, 255, 255);
        let black = ThemeColor::Rgb(0, 0, 0);
        assert_eq!(ensure_contrast(white, black, 4.5).unwrap(), white);

        let lightened = ensure_contrast(
            ThemeColor::Rgb(40, 40, 40),
            ThemeColor::Rgb(20, 20, 20),
            4.5,
        )
        .unwrap();
        assert!(contrast(lightened, ThemeColor::Rgb(20, 20, 20)).unwrap() >= 4.5);
        let darkened = ensure_contrast(
            ThemeColor::Rgb(220, 220, 220),
            ThemeColor::Rgb(240, 240, 240),
            4.5,
        )
        .unwrap();
        assert!(contrast(darkened, ThemeColor::Rgb(240, 240, 240)).unwrap() >= 4.5);
        assert!(ensure_contrast(ThemeColor::Named(NamedColor::White), black, 4.5).is_err());
    }

    #[test]
    fn mix_preserves_endpoints_and_rounds_the_midpoint() {
        let from = ThemeColor::Rgb(10, 20, 30);
        let to = ThemeColor::Rgb(110, 220, 130);

        assert_eq!(mix(from, to, 0.0).unwrap(), from);
        assert_eq!(mix(from, to, 1.0).unwrap(), to);
        assert_eq!(mix(from, to, 0.5).unwrap(), ThemeColor::Rgb(60, 120, 80));
    }
}
