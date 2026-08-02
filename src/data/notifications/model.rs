use std::fmt;

use iced::widget::image;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
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

#[derive(Debug, Clone)]
pub struct NotificationData {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<NotifAction>,
    pub urgency: Urgency,
    pub window_id_hint: Option<u64>,
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
    pub const DISMISSED: u32 = 2;
    pub const CLOSED_BY_CALL: u32 = 3;
    pub const UNDEFINED: u32 = 4;
}

#[derive(Debug, Clone)]
pub enum Outbound {
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
