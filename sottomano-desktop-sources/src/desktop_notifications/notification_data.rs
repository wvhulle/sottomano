use std::{sync::Arc, time::Duration};

use image::RgbaImage;
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

use crate::system_protocol_clients::dbus_desktop_buses::OwnerProcess;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExpireTimeout {
    Never,
    After(Duration),
    #[default]
    ServerDefault,
}

impl ExpireTimeout {
    #[must_use]
    pub fn from_millis(requested: i32) -> Self {
        match u64::try_from(requested) {
            Ok(0) => Self::Never,
            Ok(millis) => Self::After(Duration::from_millis(millis)),
            Err(_) => Self::ServerDefault,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionTint {
    Proceed,
    Hold,
    Refuse,
    Inspect,
}

impl From<&str> for ActionTint {
    fn from(action_key: &str) -> Self {
        match action_key {
            "quit" | "deny" | "end" | "reject" => ActionTint::Refuse,
            "stop" | "cancel" | "later" | "snooze" => ActionTint::Hold,
            _ => ActionTint::Proceed,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotifAction {
    pub key: String,
    pub label: String,
    pub tint: ActionTint,
}

#[derive(Debug, Clone)]
pub enum NotifImage {
    Rgba(Arc<RgbaImage>),
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SoundHint {
    #[default]
    Allowed,
    Suppressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Residency {
    #[default]
    Transient,
    Resident,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationData {
    pub id: u32,
    pub app_name: Option<String>,
    pub app_icon: Option<String>,
    pub summary: String,
    pub body: Option<String>,
    pub actions: Vec<NotifAction>,
    pub severity: Severity,
    pub expire_timeout: ExpireTimeout,
    pub window_id_hint: Option<u64>,
    pub sender: OwnerProcess,
    pub desktop_entry: Option<String>,
    pub image: Option<NotifImage>,
    pub sound_hint: SoundHint,
    pub sound_file: Option<String>,
    pub category: Option<String>,
    pub residency: Residency,
    pub progress: Option<Ratio>,
    pub reply: ReplyPrompt,
}

impl NotificationData {
    pub fn from_shell(id: u32, summary: impl Into<String>) -> Self {
        Self {
            id,
            app_name: Some("sottomano".to_owned()),
            summary: summary.into(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ReplyPrompt {
    Offered {
        placeholder: String,
        submit: String,
    },
    #[default]
    NotOffered,
}

#[derive(Debug)]
pub enum ServerEvent {
    Notify(Box<NotificationData>),
    CloseRequested(u32),
}

pub mod close_reason {
    pub const EXPIRED: u32 = 1;
    pub const DISMISSED: u32 = 2;
    pub const CLOSED_BY_CALL: u32 = 3;
    pub const UNDEFINED: u32 = 4;
}

#[derive(Debug, Clone)]
pub enum Outbound {
    ActivationToken { id: u32, token: String },
    ActionInvoked { id: u32, key: String },
    Replied { id: u32, text: String },
    Closed { id: u32, reason: u32 },
}
