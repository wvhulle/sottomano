//! User configuration from `$XDG_CONFIG_HOME/odin.toml`.

use std::path::PathBuf;

use serde::Deserialize;

pub mod font;
pub mod layout;
pub mod palette;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub palette: palette::Palette,
    pub layout: layout::Layout,
    pub font: font::FontSpec,
}

impl Config {
    pub fn chrome_radius(&self) -> f32 {
        if self.layout.floating {
            f32::from(self.layout.corner_radius_px)
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
    let base: PathBuf = std::env::var_os("HOME").unwrap().into();
    base.join(".config").join("odin.toml")
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
    font::init(config.font.name.as_deref(), config.font.text_size_px as f32);
    config
}
