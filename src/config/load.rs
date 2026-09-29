use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
    process,
};

use chrono::Locale;
use serde::Deserialize;
use toml::de;

use super::{
    chrome::Chrome,
    font::{self, FontConfig, UserFont},
    locale,
    notifications::NotificationConfig,
    palette::Palette,
    spacing::Spacing,
    tray::TrayConfig,
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
    pub tray: TrayConfig,
    pub terminal_command_prefix: Vec<String>,
    pub color_temperature_unit: String,
    pub locale: Locale,
    pub secrets: SecretPaths,
}

#[derive(Debug, Clone, Default)]
pub struct SecretPaths {
    pub openrouter_management: Option<PathBuf>,
    pub claude_code_oauth: Option<PathBuf>,
    pub gh_auth_token: Option<PathBuf>,
    pub unsplash_access_key: Option<PathBuf>,
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
    tray: TrayConfig,
    terminal: Vec<String>,
    color_temperature_unit: String,
    locale: Option<String>,
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
            tray: TrayConfig::default(),
            terminal: ["kitty", "-e"].map(String::from).to_vec(),
            color_temperature_unit: String::from("screen-temperature.service"),
            locale: None,
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
            tracing::warn!("invalid configuration {}: {e}", path.display());
            process::exit(1);
        }
    };
    let secrets = config_home().map(|dir| dir.join("vetrata"));
    let locale = match user.locale.as_deref().map(str::parse::<Locale>) {
        None => locale::from_environment(),
        Some(Ok(configured)) => configured,
        Some(Err(_)) => {
            tracing::warn!(
                "invalid configuration {}: unknown locale {:?}",
                path.display(),
                user.locale
            );
            process::exit(1);
        }
    };
    let icon_slot_height_px =
        user.chrome.outer_height_px - 2.0 * user.widgets.indicator_thickness_px;
    Config {
        palette: user.palette,
        chrome: user.chrome,
        spacing: user.spacing,
        widgets: user.widgets,
        window_list: user.window_list,
        font: font::resolve(&user.font, icon_slot_height_px),
        notifications: user.notifications,
        tray: user.tray,
        terminal_command_prefix: user.terminal,
        color_temperature_unit: user.color_temperature_unit,
        locale,
        secrets: SecretPaths {
            openrouter_management: secrets
                .as_ref()
                .map(|dir| dir.join("openrouter-management-key"))
                .filter(|path| path.is_file()),
            claude_code_oauth: secrets
                .as_ref()
                .map(|dir| dir.join("claude-code-oauth"))
                .filter(|path| path.is_file()),
            gh_auth_token: secrets
                .as_ref()
                .map(|dir| dir.join("gh-auth-token"))
                .filter(|path| path.is_file()),
            unsplash_access_key: secrets
                .as_ref()
                .map(|dir| dir.join("unsplash-access-key"))
                .filter(|path| path.is_file()),
        },
    }
}

fn home() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn config_home() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home().map(|home| home.join(".config")))
}
