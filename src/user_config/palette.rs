//! The `palette` config group. Colors are CSS Color Level 4 strings
//! (hex, `rgb()`, `rgba()`, `hsl()`, named, `oklab()`, …) parsed via the
//! `linebender/color` crate — see the crate docs for the full grammar. Defaults
//! are gruvbox-dark.

use color::{ColorSpaceTag, DynamicColor, Srgb, parse_color};
use iced::Color;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct Palette {
    #[serde(with = "css_color")]
    pub bar_background: DynamicColor,
    #[serde(with = "css_color")]
    pub inactive: DynamicColor,
    #[serde(with = "css_color")]
    pub urgent: DynamicColor,
    #[serde(with = "css_color")]
    pub warning: DynamicColor,
    #[serde(with = "css_color")]
    pub focus: DynamicColor,
    #[serde(with = "css_color")]
    pub dim: DynamicColor,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            bar_background: srgb("#504945b3"),
            inactive: srgb("#bdae93"),
            urgent: srgb("#fb4934"),
            warning: srgb("#fabd2f"),
            focus: srgb("#8ec07c"),
            dim: srgb("#FFFFFF80"),
        }
    }
}

fn srgb(s: &str) -> DynamicColor {
    parse_color(s).expect("invalid default color")
}

/// `#[serde(with = "...")]` adapter: deserialize a [`DynamicColor`] from any
/// CSS Color Level 4 string the [`color`] crate accepts (hex, `rgb()`/`rgba()`,
/// `hsl()`, named, `oklab()`, …). `DynamicColor`'s built-in serde derives from
/// a struct, so this string adapter is needed for the TOML config to be just
/// `key = "#rrggbb"`.
mod css_color {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, de::Error as _};

    use super::DynamicColor;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DynamicColor, D::Error> {
        let s = String::deserialize(d)?;
        DynamicColor::from_str(&s).map_err(D::Error::custom)
    }
}

impl Palette {
    fn to_iced(c: DynamicColor) -> Color {
        let [r, g, b, a] = c
            .convert(ColorSpaceTag::Srgb)
            .to_alpha_color::<Srgb>()
            .components;
        Color { r, g, b, a }
    }

    pub fn bar_background(&self) -> Color {
        Self::to_iced(self.bar_background)
    }

    pub fn inactive(&self) -> Color {
        Self::to_iced(self.inactive)
    }

    pub fn urgent(&self) -> Color {
        Self::to_iced(self.urgent)
    }

    pub fn warning(&self) -> Color {
        Self::to_iced(self.warning)
    }

    pub fn focus(&self) -> Color {
        Self::to_iced(self.focus)
    }

    pub fn dim(&self) -> Color {
        Self::to_iced(self.dim)
    }
}
