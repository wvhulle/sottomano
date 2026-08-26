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
    locker::Locker,
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
    pub locker: Locker,
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
    tray: TrayConfig,
    terminal: Vec<String>,
    color_temperature_unit: String,
    locker: Locker,
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
            locker: Locker::default(),
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
        tray: user.tray,
        terminal_command_prefix: user.terminal,
        color_temperature_unit: user.color_temperature_unit,
        locker: user.locker,
        secrets: SecretPaths {
            openrouter_management: secret_path(
                "VETRATA_OPENROUTER_KEY_FILE",
                [config_home().map(|dir| dir.join("vetrata/openrouter-management-key"))],
            ),
            claude_code_oauth: secret_path(
                "VETRATA_CLAUDE_CODE_OAUTH_FILE",
                [
                    home().map(|home| home.join(".claude/.credentials.json")),
                    config_home().map(|dir| dir.join("vetrata/claude-code-oauth")),
                ],
            ),
            gh_auth_token: secret_path(
                "VETRATA_GH_AUTH_TOKEN_FILE",
                [
                    state_home().map(|dir| dir.join("chatbot/copilot-token")),
                    config_home().map(|dir| dir.join("vetrata/gh-auth-token")),
                ],
            ),
        },
    }
}

fn secret_path(var: &str, defaults: impl IntoIterator<Item = Option<PathBuf>>) -> Option<PathBuf> {
    env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| defaults.into_iter().flatten().find(|path| path.is_file()))
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

fn state_home() -> Option<PathBuf> {
    env::var_os("XDG_STATE_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home().map(|home| home.join(".local/state")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_override_wins_over_defaults() {
        unsafe { env::set_var("VETRATA_TEST_SECRET", "/explicit/path") };
        let resolved = secret_path(
            "VETRATA_TEST_SECRET",
            [Some(PathBuf::from("/etc/hostname"))],
        );
        unsafe { env::remove_var("VETRATA_TEST_SECRET") };
        assert_eq!(resolved, Some(PathBuf::from("/explicit/path")));
    }

    #[test]
    fn defaults_are_skipped_unless_the_file_exists() {
        let resolved = secret_path(
            "VETRATA_TEST_UNSET_SECRET",
            [
                Some(PathBuf::from("/nonexistent/first")),
                Some(PathBuf::from("/etc/hostname")),
            ],
        );
        assert_eq!(resolved, Some(PathBuf::from("/etc/hostname")));
    }

    #[test]
    fn no_default_matches_when_none_exist() {
        let resolved = secret_path(
            "VETRATA_TEST_UNSET_SECRET",
            [Some(PathBuf::from("/nonexistent/only"))],
        );
        assert_eq!(resolved, None);
    }

    #[test]
    fn config_home_follows_xdg_then_home() {
        unsafe { env::set_var("XDG_CONFIG_HOME", "/xdg/config") };
        assert_eq!(config_home(), Some(PathBuf::from("/xdg/config")));
        unsafe { env::remove_var("XDG_CONFIG_HOME") };
        assert_eq!(config_home(), home().map(|home| home.join(".config")));
    }
}
