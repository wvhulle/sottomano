//! odin-rs's own user config: `$XDG_CONFIG_HOME/odin.toml`.
//!
//! Missing keys keep their defaults so the bar always renders something
//! usable. Style/font groups the bar doesn't render yet (notification, secret
//! files, …) are omitted; odin's QML config is no longer consumed.

use std::path::PathBuf;

use iced::Color;
use serde::Deserialize;

pub mod layout;
pub mod palette;

pub use layout::Layout;
pub use palette::Palette;

/// Icon box side — odin's `imageIconSize`. Kept as a constant since the icon
/// size is a design knob, not tied to font size (which now follows iced's
/// default).
pub const IMAGE_ICON_SIZE: u16 = 17;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub palette: Palette,
    pub layout: Layout,
}

impl Config {
    /// Outer bar-strip height — odin's `barOuterHeight`
    /// (`imageIconSize + 2 + 2 * verticalPadding`).
    pub fn bar_outer_height(&self) -> u32 {
        IMAGE_ICON_SIZE as u32 + 2 + 2 * self.layout.vertical_padding as u32
    }

    /// Bar chrome fill — the `bar_background` color straight from the config
    /// (alpha honored from the CSS string).
    pub fn chrome_color(&self) -> Color {
        self.palette.bar_background()
    }

    /// Corner radius of the chrome — rounded only when floating.
    pub fn chrome_radius(&self) -> f32 {
        if self.layout.floating {
            self.layout.corner_radius as f32
        } else {
            0.0
        }
    }
}

/// A failed config load. Missing file and malformed TOML both land here.
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

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        });
    base.join("odin.toml")
}

fn read(path: &std::path::Path) -> Result<Config, LoadError> {
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}

/// Load the config, falling back to the gruvbox-dark defaults on any failure
/// so the bar always has a usable theme.
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
