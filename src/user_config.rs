//! User configuration from `$XDG_CONFIG_HOME/odin.toml`.

use std::path::PathBuf;

use serde::Deserialize;

pub mod font;
pub mod layout;
pub mod palette;

/// The runtime config handed to the bar: same shape as the TOML, but with the
/// font resolved to a concrete [`RuntimeFont`](font::RuntimeFont). Constructed
/// by [`load`] from a [`UserConfig`]; not deserialized directly (a resolved
/// font has no `Default`/`Deserialize`).
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub palette: palette::Palette,
    pub layout: layout::Layout,
    pub font: font::RuntimeFont,
}

/// The config exactly as it appears in the TOML file on disk, before the font
/// is resolved.
#[derive(Default, Deserialize)]
#[serde(default)]
struct UserConfig {
    palette: palette::Palette,
    layout: layout::Layout,
    font: font::FontConfig,
}

impl RuntimeConfig {
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

fn read(path: &std::path::Path) -> Result<UserConfig, LoadError> {
    let text = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}

pub fn load() -> RuntimeConfig {
    let path = config_path();
    let user = match read(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {} {e}; using defaults", path.display());
            UserConfig::default()
        }
    };
    // Resolve the family + load its file up front, so the whole font context
    // rides along in `RuntimeConfig` and the first frame renders in the right
    // font.
    RuntimeConfig {
        palette: user.palette,
        layout: user.layout,
        font: font::RuntimeFont::resolve(&user.font),
    }
}
