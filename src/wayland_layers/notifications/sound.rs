//! Alert sounds for incoming notifications. odin shells out to `pw-play` (as
//! the rest of the app shells out to CLIs) rather than holding an audio handle:
//! a toast is rare and short, so a fire-and-forget player process fits. Themed
//! sound names are resolved to files by `freedesktop-sound` (the XDG Sound
//! Theme spec, the sibling of the `freedesktop-icons` lookup used elsewhere).

use std::{path::PathBuf, process::Command};

use iced::Task;

use super::model::{NotificationData, Urgency};
use crate::{app::Message, message_subscriptions::blocking};

const PLAYER: &str = "pw-play";

/// Which themed alert an incoming notification warrants. Resolved in priority
/// order in [`Alert::of`].
#[derive(Debug, Clone, Copy)]
enum Alert {
    /// Critical urgency — grab attention over everything else.
    Critical,
    /// Asks the user to act — it carries action buttons.
    Interactive,
    /// An incoming message (chat or mail).
    NewMessage,
    /// A finished transfer.
    Complete,
    /// A failure.
    Error,
    /// Purely informational.
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
            // Any category whose trailing segment is `error` (e.g. `im.error`).
            Some(c) if c.rsplit('.').next() == Some("error") => Self::Error,
            _ => Self::Passive,
        }
    }

    /// The freedesktop sound-theme name to play.
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

/// Play `data`'s alert sound off-thread: its explicit `sound-file` hint when it
/// names a real file, else the freedesktop theme sound for its kind. A missing
/// file or player just yields silence.
pub fn play(data: &NotificationData) -> Task<Message> {
    let explicit = data.sound_file.clone();
    let alert = Alert::of(data);
    blocking("notification sound", move || {
        if let Some(file) = resolve(explicit, alert) {
            let _ = Command::new(PLAYER).arg(file).status();
        }
    })
    .discard()
}

fn resolve(explicit: Option<String>, alert: Alert) -> Option<PathBuf> {
    explicit
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .or_else(|| themed(alert.theme_name()))
        // A theme missing the specific name still gets the generic message tone.
        .or_else(|| themed("message"))
}

/// The themed sound file for `name`, via the XDG Sound Theme spec (default
/// "freedesktop" theme, with the spec's parent-theme fallback).
fn themed(name: &str) -> Option<PathBuf> {
    freedesktop_sound::lookup(name).with_cache().find()
}
