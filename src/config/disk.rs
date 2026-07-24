//! Config as read from the TOML file on disk, before the font is loaded and
//! colours are converted.

use std::path::PathBuf;

use color::DynamicColor;
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct DiskConfig {
    pub palette: DiskPaletteConfig,
    pub layout: Layout,
    pub font: DiskFontConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DiskFontConfig {
    pub name: Option<String>,
    pub text_size_px: f32,
    pub icon_size_px: f32,
}

impl Default for DiskFontConfig {
    fn default() -> Self {
        Self {
            name: None,
            text_size_px: 14.0,
            icon_size_px: 15.0,
        }
    }
}

/// Colours are CSS Color Level 4 strings (hex, `rgb()`, `hsl()`, named,
/// `oklab()`, …). Defaults are gruvbox-dark.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct DiskPaletteConfig {
    #[serde(with = "css_color")]
    pub bar_background: DynamicColor,
    #[serde(with = "css_color")]
    pub bar_foreground: DynamicColor,
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

impl Default for DiskPaletteConfig {
    fn default() -> Self {
        Self {
            bar_background: srgb("transparent"),
            bar_foreground: srgb("#ffffff"),
            inactive: srgb("#bdae93"),
            urgent: srgb("#fb4934"),
            warning: srgb("#fabd2f"),
            focus: srgb("#8ec07c"),
            dim: srgb("#FFFFFF80"),
        }
    }
}

fn srgb(s: &str) -> DynamicColor {
    color::parse_color(s).expect("invalid default color")
}

/// Deserialize a colour from a CSS Color Level 4 string, so the TOML can be
/// just `key = "#rrggbb"`.
mod css_color {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, de::Error as _};

    use super::DynamicColor;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DynamicColor, D::Error> {
        let s = String::deserialize(d)?;
        DynamicColor::from_str(&s).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub corner_radius_px: u16,
    pub vertical_padding_px: f32,
    pub horizontal_group_padding_px: f32,
    pub horizontal_widget_spacing_px: f32,
    pub horizontal_group_spacing_px: f32,
    pub floating: bool,
    pub blur: bool,
    pub shadow: bool,
    pub float_offset_px: f32,
    /// Track width of a settings-panel slider (session UI chrome, not a bar
    /// element).
    pub slider_width_px: f32,
    pub outer_height_px: f32,
    pub focus_underline_thickness_px: f32,
    pub window_segment_thickness_px: f32,
    pub menu_expand_ms: u64,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            corner_radius_px: 8,
            vertical_padding_px: 3.0,
            horizontal_group_padding_px: 5.0,
            horizontal_widget_spacing_px: 3.0,
            horizontal_group_spacing_px: 16.0,
            floating: true,
            blur: true,
            shadow: false,
            float_offset_px: 0.0,
            slider_width_px: 60.0,
            outer_height_px: 28.0,
            focus_underline_thickness_px: 3.4,
            window_segment_thickness_px: 2.0,
            menu_expand_ms: 90,
        }
    }
}

#[derive(Debug)]
pub enum LoadError {
    Read(std::io::Error),
    Parse(toml::de::Error),
}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Read(e)
    }
}

impl From<toml::de::Error> for LoadError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(e) => write!(f, "read failed: {e}"),
            Self::Parse(e) => write!(f, "parse failed: {e}"),
        }
    }
}

pub fn config_path() -> PathBuf {
    let base: PathBuf = std::env::var_os("HOME").unwrap().into();
    base.join(".config").join("odin.toml")
}

pub fn read(path: &std::path::Path) -> Result<DiskConfig, LoadError> {
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}
