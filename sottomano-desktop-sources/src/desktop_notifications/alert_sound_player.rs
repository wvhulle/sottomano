use std::{path::PathBuf, process::Command};

use sottomano_toml_config::severity::Severity;

use crate::{
    blocking_background_work::blocking, desktop_notifications::notification_data::NotificationData,
};

const PLAYER: &str = "pw-play";

#[derive(Debug, Clone, Copy)]
enum Alert {
    Critical,
    Interactive,
    NewMessage,
    Complete,
    Error,
    Passive,
}

impl Alert {
    fn of(data: &NotificationData) -> Self {
        if matches!(data.severity, Severity::Critical) {
            return Self::Critical;
        }
        if !data.actions.is_empty() {
            return Self::Interactive;
        }
        match data.category.as_deref() {
            Some(c) if c.starts_with("email") || c.starts_with("im") => Self::NewMessage,
            Some("transfer.complete") => Self::Complete,
            Some(c) if c.rsplit('.').next() == Some("error") => Self::Error,
            _ => Self::Passive,
        }
    }

    fn theme_name(self) -> &'static str {
        match self {
            Self::Critical => "window-attention",
            Self::Interactive => "window-question",
            Self::NewMessage => "message-new-instant",
            Self::Complete => "complete",
            Self::Error => "dialog-error",
            Self::Passive => "message",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum SoundTheme {
    #[default]
    Unconfigured,
    Named(String),
}

impl SoundTheme {
    #[must_use]
    pub fn read_session() -> Self {
        let configured = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.sound", "theme-name"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok());
        match configured
            .as_deref()
            .map(|value| value.trim().trim_matches('\''))
        {
            Some(theme) if !theme.is_empty() => Self::Named(theme.to_owned()),
            Some(_) | None => Self::Unconfigured,
        }
    }

    fn find(&self, name: &str) -> Option<PathBuf> {
        let lookup = freedesktop_sound::lookup(name).with_cache();
        match self {
            Self::Named(theme) => lookup.with_theme(theme).find(),
            Self::Unconfigured => lookup.find(),
        }
    }
}

pub fn play(data: &NotificationData, theme: &SoundTheme) -> impl Future<Output = ()> + use<> {
    let explicit = data.sound_file.clone();
    let alert = Alert::of(data);
    let theme = theme.clone();
    blocking("notification sound", move || {
        if let Some(file) = resolve(explicit, alert, &theme) {
            let _ = Command::new(PLAYER).arg(file).status();
        }
    })
}

fn resolve(explicit: Option<String>, alert: Alert, theme: &SoundTheme) -> Option<PathBuf> {
    explicit
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| theme.find(alert.theme_name()))
        .or_else(|| theme.find("message"))
}
