//! Notification data model and the two message types crossing the D-Bus bridge.

use iced::widget::image;
use tokio::sync::mpsc;

/// freedesktop urgency levels (the `urgency` byte hint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Urgency {
    Low,
    #[default]
    Normal,
    Critical,
}

/// One `--action` the client offered: the key it wants back and the label to
/// draw.
#[derive(Debug, Clone)]
pub struct NotifAction {
    pub key: String,
    pub label: String,
}

/// A notification's own image (the `image-data`/`image-path` hint), distinct
/// from the application icon. `Named` is a path or an icon name still to be
/// resolved by the card.
#[derive(Debug, Clone)]
pub enum NotifImage {
    Rgba(image::Handle),
    Named(String),
}

/// One notification as received over D-Bus, resolved into render-ready data.
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
    /// The `suppress-sound` hint: the client asks that no alert sound play for
    /// this notification even when sounds are on.
    pub suppress_sound: bool,
    /// The `sound-file` hint: an explicit sound file to play instead of the
    /// theme default.
    pub sound_file: Option<String>,
    /// The freedesktop `category` hint (e.g. `email.arrived`, `im.received`),
    /// used to pick a more fitting alert sound.
    pub category: Option<String>,
    pub resident: bool,
}

/// D-Bus → app: what the notification server received from a client.
#[derive(Debug)]
pub enum ServerEvent {
    Notify(NotificationData),
    CloseRequested(u32),
}

/// The reason codes the `NotificationClosed` signal carries. There is no
/// timeout-expiry reason (1): a toast is only ever closed by an event.
pub mod close_reason {
    pub const DISMISSED: u32 = 2;
    pub const CLOSED_BY_CALL: u32 = 3;
    pub const UNDEFINED: u32 = 4;
}

/// App → D-Bus: signals the notification server must emit back to clients.
#[derive(Debug, Clone)]
pub enum Outbound {
    ActionInvoked { id: u32, key: String },
    Closed { id: u32, reason: u32 },
}

/// The app's handle to the D-Bus side, delivered once at startup via
/// [`NotificationMessage::Connected`]. Opaque and cheap to clone; the app sends
/// [`Outbound`] through it to emit signals.
///
/// [`NotificationMessage::Connected`]: crate::wayland_layers::notifications::NotificationMessage::Connected
#[derive(Clone)]
pub struct NotifHandle(mpsc::Sender<Outbound>);

impl NotifHandle {
    pub fn new(tx: mpsc::Sender<Outbound>) -> Self {
        Self(tx)
    }

    /// A clone of the outbound sender, to move into a fire-and-forget task.
    pub fn sender(&self) -> mpsc::Sender<Outbound> {
        self.0.clone()
    }
}

impl std::fmt::Debug for NotifHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NotifHandle")
    }
}
