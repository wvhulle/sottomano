use std::{path::PathBuf, process::Command};

use iced::Task;

use crate::{
    data::notifications::model::{NotificationData, Urgency},
    message_subscriptions::blocking,
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
        if matches!(data.urgency, Urgency::Critical) {
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

pub(crate) fn session_theme() -> Option<String> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.sound", "theme-name"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let theme = value.trim().trim_matches('\'');
    (!theme.is_empty()).then(|| theme.to_owned())
}

pub fn play<Message: Send + 'static>(
    data: &NotificationData,
    theme: Option<&str>,
) -> Task<Message> {
    let explicit = data.sound_file.clone();
    let alert = Alert::of(data);
    let theme = theme.map(ToOwned::to_owned);
    blocking("notification sound", move || {
        if let Some(file) = resolve(explicit, alert, theme.as_deref()) {
            let _ = Command::new(PLAYER).arg(file).status();
        }
    })
    .discard()
}

fn resolve(explicit: Option<String>, alert: Alert, theme: Option<&str>) -> Option<PathBuf> {
    explicit
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| themed(alert.theme_name(), theme))
        .or_else(|| themed("message", theme))
}

fn themed(name: &str, theme: Option<&str>) -> Option<PathBuf> {
    let lookup = freedesktop_sound::lookup(name).with_cache();
    match theme {
        Some(theme) => lookup.with_theme(theme).find(),
        None => lookup.find(),
    }
}
