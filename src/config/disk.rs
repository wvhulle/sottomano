use std::path::PathBuf;

use color::DynamicColor;
use serde::Deserialize;

use crate::data::source::niri::{HEdge, VEdge};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

pub struct Edges {
    pub vertical: VEdge,
    pub horizontal: HEdge,
}

impl Corner {
    pub fn edges(self) -> Edges {
        let (vertical, horizontal) = match self {
            Self::TopLeft => (VEdge::Top, HEdge::Left),
            Self::TopCenter => (VEdge::Top, HEdge::Center),
            Self::TopRight => (VEdge::Top, HEdge::Right),
            Self::BottomLeft => (VEdge::Bottom, HEdge::Left),
            Self::BottomCenter => (VEdge::Bottom, HEdge::Center),
            Self::BottomRight => (VEdge::Bottom, HEdge::Right),
        };
        Edges {
            vertical,
            horizontal,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct DiskConfig {
    pub palette: DiskPaletteConfig,
    pub layout: Layout,
    pub font: DiskFontConfig,
    pub notifications: NotificationConfig,
    pub tray: TrayConfig,
    pub terminal: Vec<String>,
}

impl Default for DiskConfig {
    fn default() -> Self {
        Self {
            palette: DiskPaletteConfig::default(),
            layout: Layout::default(),
            font: DiskFontConfig::default(),
            notifications: NotificationConfig::default(),
            tray: TrayConfig::default(),
            terminal: ["kitty", "-e"].map(String::from).to_vec(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TrayConfig {
    pub deny_ids: Vec<String>,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            deny_ids: ["nm-applet", "blueman", "spotify"]
                .map(String::from)
                .to_vec(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub point_at_source: bool,
    pub fallback_position: Corner,
    pub edge_gap: f32,
    pub card_width: f32,
    pub app_icon_size: f32,
    pub image_size: f32,
    pub badge_size: f32,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            point_at_source: true,
            fallback_position: Corner::BottomRight,
            edge_gap: 8.0,
            card_width: 380.0,
            app_icon_size: 40.0,
            image_size: 48.0,
            badge_size: 20.0,
        }
    }
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
            icon_size_px: 16.0,
        }
    }
}

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
    pub horizontal_group_padding_px: f32,
    pub horizontal_widget_spacing_px: f32,
    pub horizontal_group_spacing_px: f32,
    pub floating: bool,
    pub blur_background: bool,
    pub disconnected_groups: bool,
    pub shadow: bool,
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
            horizontal_group_padding_px: 5.0,
            horizontal_widget_spacing_px: 5.0,
            horizontal_group_spacing_px: 16.0,
            floating: false,
            blur_background: false,
            disconnected_groups: false,
            shadow: true,
            slider_width_px: 60.0,
            outer_height_px: 23.0,
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
    base.join(".config").join("vetrata.toml")
}

pub fn read(path: &std::path::Path) -> Result<DiskConfig, LoadError> {
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}
