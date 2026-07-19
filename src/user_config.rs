//! User configuration from `$XDG_CONFIG_HOME/odin.toml`.

use std::path::PathBuf;

use iced::Color;
use serde::Deserialize;

pub mod font;
pub mod layout;
pub mod palette;

pub use font::FontSpec;
pub use layout::Layout;
pub use palette::Palette;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub palette: Palette,
    pub layout: Layout,
    pub font: FontSpec,
}

impl Config {
    pub fn chrome_color(&self) -> Color {
        self.palette.bar_background()
    }

    pub fn chrome_radius(&self) -> f32 {
        if self.layout.floating {
            f32::from(self.layout.corner_radius)
        } else {
            0.0
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

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map_or_else(
        || {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        },
        PathBuf::from,
    );
    base.join("odin.toml")
}

fn read(path: &std::path::Path) -> Result<Config, LoadError> {
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}

pub fn load() -> Config {
    let path = config_path();
    let config = match read(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {} {e}; using defaults", path.display());
            Config::default()
        }
    };
    font::init(config.font.name.as_deref(), config.font.size_px as f32);
    config
}
