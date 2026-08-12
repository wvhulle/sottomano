use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
    process,
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
    window_list::WindowListConfig,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub palette: Palette,
    pub chrome: Chrome,
    pub spacing: Spacing,
    pub widgets: Widgets,
    pub window_list: WindowListConfig,
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
#[serde(default, deny_unknown_fields)]
struct UserConfig {
    palette: Palette,
    chrome: Chrome,
    spacing: Spacing,
    widgets: Widgets,
    window_list: WindowListConfig,
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
            window_list: WindowListConfig::default(),
            font: UserFont::default(),
            notifications: NotificationConfig::default(),
            tray: TrayToml::default(),
            terminal: ["kitty", "-e"].map(String::from).to_vec(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
    Missing,
    Unreadable(io::Error),
    Invalid(de::Error),
}

impl From<io::Error> for LoadError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::NotFound {
            Self::Missing
        } else {
            Self::Unreadable(e)
        }
    }
}

impl From<de::Error> for LoadError {
    fn from(e: de::Error) -> Self {
        Self::Invalid(e)
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => write!(f, "file not found"),
            Self::Unreadable(e) => write!(f, "cannot be read: {e}"),
            Self::Invalid(e) => write!(f, "\n{e}"),
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
        Ok(user) => user,
        Err(LoadError::Missing) => UserConfig::default(),
        Err(e) => {
            eprintln!("invalid configuration {}: {e}", path.display());
            process::exit(1);
        }
    };
    Config {
        palette: user.palette,
        chrome: user.chrome,
        spacing: user.spacing,
        widgets: user.widgets,
        window_list: user.window_list,
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
