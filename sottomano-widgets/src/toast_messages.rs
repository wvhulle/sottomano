use sottomano_sources::notifications::model::{NotificationData, Outbound, ServerEvent};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum ReplyMessage {
    PointerEntered,
    PointerLeft,
    Edited(String),
    Submitted,
}

#[derive(Debug, Clone)]
pub enum NotificationMessage {
    Connected(mpsc::Sender<Outbound>),
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
