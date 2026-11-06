use sottomano_desktop_sources::desktop_notifications::{
    freedesktop_notification_server::NotificationSignals,
    notification_data::{NotificationData, ServerEvent},
};

#[derive(Debug, Clone)]
pub enum ReplyMessage {
    PointerEntered,
    PointerLeft,
    Edited(String),
    Submitted,
}

#[derive(Debug, Clone)]
pub enum NotificationMessage {
    Connected(NotificationSignals),
    Served(Box<NotificationData>),
    CloseRequested(u32),
    BodyClicked(u32),
    HoverChanged { id: u32, hovered: bool },
    Dismiss(u32),
    ActionClicked { id: u32, key: String },
    DefaultActivated { id: u32, token: Option<String> },
    Reply { id: u32, message: ReplyMessage },
    OpenFile(u32),
    CopyFilePath(u32),
    ExpiryDue,
}

impl From<ServerEvent> for NotificationMessage {
    fn from(event: ServerEvent) -> Self {
        match event {
            ServerEvent::Notify(data) => Self::Served(data),
            ServerEvent::CloseRequested(id) => Self::CloseRequested(id),
        }
    }
}
