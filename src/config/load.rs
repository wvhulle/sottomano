use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

use chrono::Locale;
use serde::Deserialize;
use toml::de;

use super::{
    bar_size::BarSize,
    font::{self, FontChoice, FontError, ResolvedFont},
    locale,
    palette::Palette,
    spacing::Spacing,
    toasts::ToastConfig,
    tray::TrayConfig,
    widget_sizes::WidgetSizes,
    window_list::WindowListConfig,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub palette: Palette,
    pub bar: BarSize,
    pub spacing: Spacing,
    pub widget_sizes: WidgetSizes,
    pub window_list: WindowListConfig,
    pub font: ResolvedFont,
    pub toasts: ToastConfig,
    pub tray: TrayConfig,
    pub terminal_command_prefix: Vec<String>,
    pub color_temperature_service: String,
    pub locale: Locale,
}

impl Config {
    pub fn fallback() -> Self {
        UserConfig::default()
            .resolve()
            .expect("the default configuration resolves")
    }
}

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct UserConfig {
    palette: Palette,
    bar: BarSize,
    spacing: Spacing,
    widget_sizes: WidgetSizes,
    window_list: WindowListConfig,
    font: FontChoice,
    toasts: ToastConfig,
    tray: TrayConfig,
    terminal_command_prefix: Vec<String>,
    color_temperature_service: String,
    locale: Option<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            palette: Palette::default(),
            bar: BarSize::default(),
            spacing: Spacing::default(),
            widget_sizes: WidgetSizes::default(),
            window_list: WindowListConfig::default(),
            font: FontChoice::default(),
            toasts: ToastConfig::default(),
            tray: TrayConfig::default(),
            terminal_command_prefix: ["kitty", "-e"].map(String::from).to_vec(),
            color_temperature_service: String::from("screen-temperature.service"),
            locale: None,
        }
    }
}

impl UserConfig {
    fn resolve(self) -> Result<Config, LoadError> {
        let locale = match self.locale {
            None => locale::from_environment(),
            Some(configured) => configured
                .parse::<Locale>()
                .map_err(|_| LoadError::UnknownLocale(configured))?,
        };
        let icon_slot_height_px =
            self.bar.height_px - 2.0 * self.widget_sizes.indicator_thickness_px;
        Ok(Config {
            font: font::resolve(&self.font, icon_slot_height_px).map_err(LoadError::Font)?,
            palette: self.palette,
            bar: self.bar,
            spacing: self.spacing,
            widget_sizes: self.widget_sizes,
            window_list: self.window_list,
            toasts: self.toasts,
            tray: self.tray,
            terminal_command_prefix: self.terminal_command_prefix,
            color_temperature_service: self.color_temperature_service,
            locale,
        })
    }
}

#[derive(Debug)]
pub enum LoadError {
    Unreadable(io::Error),
    Invalid(de::Error),
    UnknownLocale(String),
    Font(FontError),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(e) => write!(f, "cannot be read: {e}"),
            Self::Invalid(e) => write!(f, "{e}"),
            Self::UnknownLocale(locale) => write!(f, "unknown locale {locale:?}"),
            Self::Font(e) => write!(f, "{e}"),
        }
    }
}

#[derive(Debug, Clone, Hash)]
pub struct ConfigFile {
    pub(super) path: PathBuf,
}

impl ConfigFile {
    pub fn in_config_home(config_home: &Path) -> Self {
        Self {
            path: config_home.join("vetrata.toml"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Config, LoadError> {
        self.read()?.resolve()
    }

    fn read(&self) -> Result<UserConfig, LoadError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => toml::from_str(&text).map_err(LoadError::Invalid),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(UserConfig::default()),
            Err(e) => Err(LoadError::Unreadable(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, process};

    use super::{ConfigFile, LoadError};

    fn file_containing(test: &str, toml: &str) -> ConfigFile {
        let directory = std::env::temp_dir().join(format!("vetrata-{test}-{}", process::id()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("vetrata.toml"), toml).unwrap();
        ConfigFile::in_config_home(&directory)
    }

    #[test]
    fn keys_from_before_the_rename_are_rejected() {
        let file = file_containing("old-keys", "[chrome]\nouter_height_px = 28.0\n");
        let error = file.load().unwrap_err();
        fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
        assert!(matches!(error, LoadError::Invalid(_)));
        assert!(error.to_string().contains("chrome"));
    }

    #[test]
    fn an_unknown_locale_is_rejected_with_its_name() {
        let file = file_containing("locale", "locale = \"xx_nowhere\"\n");
        let error = file.load().unwrap_err();
        fs::remove_dir_all(file.path().parent().unwrap()).unwrap();
        assert!(matches!(error, LoadError::UnknownLocale(ref name) if name == "xx_nowhere"));
    }
}

pub fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .unwrap_or_default()
}
