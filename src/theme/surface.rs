use crate::error::{Result, SnipError};

use super::color::contrast;
use super::{Appearance, ThemeColor, mix};

pub const BAR_CONTRAST_FLOOR: f64 = 1.35;
pub const PILL_CONTRAST_FLOOR: f64 = 1.5;
const MIX_STEP: f64 = 0.005;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BarPillSurfaces {
    pub bar_bg: ThemeColor,
    pub pill_secondary: ThemeColor,
}

/// Build the neutral surface ladder used by bars and secondary pills.
///
/// Light themes move toward black and dark themes move toward white, so both
/// appearances keep the same perceptual order: canvas, bar, then pill.
pub fn derive_bar_pill_surfaces(
    bar_source: ThemeColor,
    canvas: ThemeColor,
    appearance: Appearance,
) -> Result<BarPillSurfaces> {
    let bar_bg = stepped_surface(bar_source, canvas, appearance, BAR_CONTRAST_FLOOR)?;
    let pill_secondary = stepped_surface(bar_bg, bar_bg, appearance, PILL_CONTRAST_FLOOR)?;
    Ok(BarPillSurfaces {
        bar_bg,
        pill_secondary,
    })
}

fn stepped_surface(
    color: ThemeColor,
    reference: ThemeColor,
    appearance: Appearance,
    contrast_floor: f64,
) -> Result<ThemeColor> {
    let current = contrast(color, reference).ok_or_else(|| {
        SnipError::validation(format!(
            "cannot derive surfaces from {} against {}: both colors must be #rrggbb",
            color.as_string(),
            reference.as_string()
        ))
    })?;
    if current >= contrast_floor {
        return Ok(color);
    }
    let target = match appearance {
        Appearance::Dark => ThemeColor::Rgb(255, 255, 255),
        Appearance::Light => ThemeColor::Rgb(0, 0, 0),
    };
    for step in 1..=200 {
        let candidate = mix(color, target, f64::from(step) * MIX_STEP)?;
        if contrast(candidate, reference).expect("mixed RGB colors have contrast") >= contrast_floor
        {
            return Ok(candidate);
        }
    }
    Err(SnipError::validation(format!(
        "cannot adjust {} to contrast {contrast_floor:.2} against {}",
        color.as_string(),
        reference.as_string()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_the_same_canvas_bar_pill_ladder_for_both_appearances() {
        for (source, canvas, appearance) in [
            (
                ThemeColor::Rgb(20, 20, 20),
                ThemeColor::Rgb(10, 10, 10),
                Appearance::Dark,
            ),
            (
                ThemeColor::Rgb(245, 245, 245),
                ThemeColor::Rgb(255, 255, 255),
                Appearance::Light,
            ),
        ] {
            let surfaces = derive_bar_pill_surfaces(source, canvas, appearance).unwrap();
            assert!(contrast(surfaces.bar_bg, canvas).unwrap() >= BAR_CONTRAST_FLOOR);
            assert!(
                contrast(surfaces.pill_secondary, surfaces.bar_bg).unwrap() >= PILL_CONTRAST_FLOOR
            );
            assert!(
                contrast(surfaces.pill_secondary, canvas).unwrap()
                    > contrast(surfaces.bar_bg, canvas).unwrap()
            );
        }
    }

    #[test]
    fn requires_rgb_sources_and_canvas() {
        assert!(
            derive_bar_pill_surfaces(
                ThemeColor::Terminal,
                ThemeColor::Rgb(255, 255, 255),
                Appearance::Light,
            )
            .unwrap_err()
            .to_string()
            .contains("both colors must be #rrggbb")
        );
    }
}
