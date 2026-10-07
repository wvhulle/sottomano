use std::{fmt, time::Duration};

use iced::widget::image;
use tokio::sync::mpsc;

use crate::data::{severity::Severity, source::bus::OwnerProcess};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExpireTimeout {
    Never,
    After(Duration),
    #[default]
    ServerDefault,
}

impl ExpireTimeout {
    pub fn from_millis(requested: i32) -> Self {
        match u64::try_from(requested) {
            Ok(0) => Self::Never,
            Ok(millis) => Self::After(Duration::from_millis(millis)),
            Err(_) => Self::ServerDefault,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NotifAction {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub enum NotifImage {
    Rgba(image::Handle),
    Named(String),
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
    pub suppress_sound: bool,
    pub sound_file: Option<String>,
    pub category: Option<String>,
    pub resident: bool,
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
    Closed { id: u32, reason: u32 },
}

#[derive(Clone)]
pub struct NotifHandle(mpsc::Sender<Outbound>);

impl NotifHandle {
    pub fn new(tx: mpsc::Sender<Outbound>) -> Self {
        Self(tx)
    }

    pub fn sender(&self) -> mpsc::Sender<Outbound> {
        self.0.clone()
    }
}

impl fmt::Debug for NotifHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NotifHandle")
    }
}
