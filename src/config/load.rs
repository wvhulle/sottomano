use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use toml::de;

use super::{
    chrome::Chrome,
    font::{self, FontConfig, UserFont},
    notifications::NotificationConfig,
    palette::Palette,
    spacing::Spacing,
    widgets::Widgets,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub palette: Palette,
    pub chrome: Chrome,
    pub spacing: Spacing,
    pub widgets: Widgets,
    pub font: FontConfig,
    pub notifications: NotificationConfig,
    pub tray_deny_ids: Vec<String>,
    pub terminal_command_prefix: Vec<String>,
    pub secrets: SecretPaths,
}

#[derive(Debug, Clone, Default)]
pub struct SecretPaths {
    pub openrouter_management: Option<PathBuf>,
    pub claude_code_oauth: Option<PathBuf>,
    pub gh_auth_token: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(default)]
struct UserConfig {
    palette: Palette,
    chrome: Chrome,
    spacing: Spacing,
    widgets: Widgets,
    font: UserFont,
    notifications: NotificationConfig,
    tray: TrayToml,
    terminal: Vec<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            palette: Palette::default(),
            chrome: Chrome::default(),
            spacing: Spacing::default(),
            widgets: Widgets::default(),
            font: UserFont::default(),
            notifications: NotificationConfig::default(),
            tray: TrayToml::default(),
            terminal: ["kitty", "-e"].map(String::from).to_vec(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct TrayToml {
    deny_ids: Vec<String>,
}

impl Default for TrayToml {
    fn default() -> Self {
        Self {
            deny_ids: ["nm-applet", "blueman", "spotify"]
                .map(String::from)
                .to_vec(),
        }
    }
}

#[derive(Debug)]
pub enum LoadError {
    Read(io::Error),
    Parse(de::Error),
}

impl From<io::Error> for LoadError {
    fn from(e: io::Error) -> Self {
        Self::Read(e)
    }
}

impl From<de::Error> for LoadError {
    fn from(e: de::Error) -> Self {
        Self::Parse(e)
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(e) => write!(f, "read failed: {e}"),
            Self::Parse(e) => write!(f, "parse failed: {e}"),
        }
    }
}

fn config_path() -> PathBuf {
    let base: PathBuf = env::var_os("HOME").unwrap().into();
    base.join(".config").join("vetrata.toml")
}

fn read(path: &Path) -> Result<UserConfig, LoadError> {
    let text = fs::read_to_string(path)?;
    Ok(toml::from_str(&text)?)
}

pub fn load() -> Config {
    let path = config_path();
    let user = match read(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config: {} {e}; using defaults", path.display());
            UserConfig::default()
        }
    };
    Config {
        palette: user.palette,
        chrome: user.chrome,
        spacing: user.spacing,
        widgets: user.widgets,
        font: font::resolve(&user.font),
        notifications: user.notifications,
        tray_deny_ids: user
            .tray
            .deny_ids
            .iter()
            .map(|id| id.to_lowercase())
            .collect(),
        terminal_command_prefix: user.terminal,
        secrets: SecretPaths {
            openrouter_management: secret_path("VETRATA_OPENROUTER_KEY_FILE"),
            claude_code_oauth: secret_path("VETRATA_CLAUDE_CODE_OAUTH_FILE"),
            gh_auth_token: secret_path("VETRATA_GH_AUTH_TOKEN_FILE"),
        },
    }
}

fn secret_path(var: &str) -> Option<PathBuf> {
    env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
