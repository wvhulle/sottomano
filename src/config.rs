//! Odin's user config, consumed verbatim. For now odin-rs reads the *same*
//! `$XDG_CONFIG_HOME/odin/config.json` odin does (`../odin/UserConfig/Config.qml` is the
//! schema); later we can split to our own file. Only the styling groups odin-rs currently
//! renders — `palette`, `font`, `layout` — are modelled; missing keys keep odin's exact
//! gruvbox-dark defaults, matching QML's per-property `JsonObject` fallbacks.

use std::path::PathBuf;

use iced::Color;
use serde::Deserialize;

/// A `#rrggbb` / `#rrggbbaa` color, as odin writes them in config.json.
#[derive(Debug, Clone, Copy)]
pub struct Rgb(pub Color);

impl Rgb {
    const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
    }

    /// This color with its alpha replaced — mirrors `ConfigLoader.withBgOpacity`.
    pub fn with_alpha(self, alpha: f32) -> Color {
        Color { a: alpha, ..self.0 }
    }

    fn parse(hex: &str) -> Option<Color> {
        let digits = hex.strip_prefix('#')?;
        let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
        match digits.len() {
            6 => Some(Color::from_rgb8(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Color::from_rgba8(
                byte(0)?,
                byte(2)?,
                byte(4)?,
                byte(6)? as f32 / 255.0,
            )),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let hex = String::deserialize(d)?;
        let color = Rgb::parse(&hex)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid color {hex:?}")))?;
        Ok(Rgb(color))
    }
}

/// The `palette` group. Defaults are odin's gruvbox-dark base16 (see `ColorPalette.qml`).
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Palette {
    pub bg: Rgb,
    pub surface: Rgb,
    pub text: Rgb,
    pub muted: Rgb,
    pub critical_tint: Rgb,
    pub time_tint: Rgb,
    pub brightness_tint: Rgb,
    pub network_tint: Rgb,
    pub power_tint: Rgb,
    pub system_tint: Rgb,
    pub audio_tint: Rgb,
    pub gradient: Rgb,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            bg: Rgb::rgb(0x1d, 0x20, 0x21),
            surface: Rgb::rgb(0x50, 0x49, 0x45),
            text: Rgb::rgb(0xd5, 0xc4, 0xa1),
            muted: Rgb::rgb(0xbd, 0xae, 0x93),
            critical_tint: Rgb::rgb(0xfb, 0x49, 0x34),
            time_tint: Rgb::rgb(0xfe, 0x80, 0x19),
            brightness_tint: Rgb::rgb(0xfa, 0xbd, 0x2f),
            network_tint: Rgb::rgb(0xb8, 0xbb, 0x26),
            power_tint: Rgb::rgb(0x8e, 0xc0, 0x7c),
            system_tint: Rgb::rgb(0x83, 0xa5, 0x98),
            audio_tint: Rgb::rgb(0xd3, 0x86, 0x9b),
            gradient: Rgb::rgb(0x0b, 0x0e, 0x0f),
        }
    }
}

/// The `font` group (`FontConfig.qml`).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Font {
    pub family: String,
    pub mono: String,
    pub size: u16,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_string(),
            mono: "monospace".to_string(),
            size: 12,
        }
    }
}

/// The `layout` group (`LayoutConfig.qml`). Only the chrome-relevant knobs so far.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Layout {
    pub bg_opacity: f32,
    pub corner_radius: u16,
    pub vertical_padding: u16,
    pub horizontal_group_padding: u16,
    pub horizontal_widget_spacing: u16,
    pub horizontal_group_spacing: u16,
    pub floating: bool,
    pub blur: bool,
    pub shadow: bool,
    pub float_offset: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            bg_opacity: 0.70,
            corner_radius: 8,
            vertical_padding: 3,
            horizontal_group_padding: 6,
            horizontal_widget_spacing: 5,
            horizontal_group_spacing: 9,
            floating: true,
            blur: true,
            shadow: false,
            float_offset: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub palette: Palette,
    pub font: Font,
    pub layout: Layout,
}

impl Config {
    /// Icon box side, `round(font.size * 1.4)` — odin's `imageIconSize`.
    pub fn image_icon_size(&self) -> u16 {
        (self.font.size as f32 * 1.4).round() as u16
    }

    /// Outer bar-strip height — odin's `barOuterHeight`
    /// (`imageIconSize + 2 + 2 * verticalPadding`).
    pub fn bar_outer_height(&self) -> u32 {
        (self.image_icon_size() + 2 + 2 * self.layout.vertical_padding) as u32
    }

    /// Bar chrome fill: `surface` at the configured background opacity. At `bgOpacity 0`
    /// this is fully transparent and only a backdrop (if any) shows through.
    pub fn chrome_color(&self) -> Color {
        self.palette.surface.with_alpha(self.layout.bg_opacity)
    }

    /// Corner radius of the chrome — rounded only when floating, like odin.
    pub fn chrome_radius(&self) -> f32 {
        if self.layout.floating {
            self.layout.corner_radius as f32
        } else {
            0.0
        }
    }
}

/// A failed config load. `read_to_string` follows the dark/light symlink odin points
/// `config.json` at, so a missing target or malformed JSON both land here.
#[derive(Debug)]
pub enum LoadError {
    Read(std::io::Error),
    Parse(serde_json::Error),
}

impl From<std::io::Error> for LoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Read(e)
    }
}

impl From<serde_json::Error> for LoadError {
    fn from(e: serde_json::Error) -> Self {
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

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        });
    base.join("odin").join("config.json")
}

fn read(path: &std::path::Path) -> Result<Config, LoadError> {
    let text = std::fs::read_to_string(path)?;
    let config = serde_json::from_str(&text)?;
    Ok(config)
}

/// Load odin's config, falling back to the gruvbox-dark defaults on any failure so the
/// bar always has a usable theme (odin's own behaviour: missing keys keep defaults).
pub fn load() -> Config {
    let path = config_path();
    match read(&path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("config: {} {e}; using defaults", path.display());
            Config::default()
        }
    }
}
