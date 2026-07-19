//! Odin's user config, consumed verbatim. For now odin-rs reads the *same*
//! `$XDG_CONFIG_HOME/odin/config.json` odin does
//! (`../odin/UserConfig/Config.qml` is the schema); later we can split to our
//! own file. Only the styling groups odin-rs currently renders — `palette`,
//! `font`, `layout` — are modelled; missing keys keep odin's exact gruvbox-dark
//! defaults, matching QML's per-property `JsonObject` fallbacks.

use std::path::PathBuf;

use iced::Color;
use serde::Deserialize;

pub mod color_palette;
pub mod font_config;
pub mod layout_config;

pub use color_palette::Palette;
pub use font_config::FontConfig;
pub use layout_config::Layout;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Raw {
    pub palette: Palette,
    pub font: font_config::Spec,
    pub layout: Layout,
}

impl Raw {
    fn resolve(self) -> Config {
        Config {
            palette: self.palette,
            font: self.font.resolve(),
            layout: self.layout,
        }
    }
}

/// The runtime config: every string-typed font alias has been resolved into a
/// concrete [`iced::Font`] by [`font_config::Spec::resolve`].
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub palette: Palette,
    pub font: FontConfig,
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

    /// Bar chrome fill: `surface` at the configured background opacity. At
    /// `bgOpacity 0` this is fully transparent and only a backdrop (if any)
    /// shows through.
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

/// A failed config load. `read_to_string` follows the dark/light symlink odin
/// points `config.json` at, so a missing target or malformed JSON both land
/// here.
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
    let config: Raw = serde_json::from_str(&text)?;
    Ok(config.resolve())
}

/// Load odin's config, falling back to the gruvbox-dark defaults on any failure
/// so the bar always has a usable theme (odin's own behaviour: missing keys
/// keep defaults).
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