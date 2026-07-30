//! The `org.freedesktop.Notifications` D-Bus server and its bridge to the app.
//!
//! The zbus side owns only D-Bus identity (the id counter and the connection to
//! emit signals through); the app owns the render truth. Two channels cross the
//! bridge: [`ServerEvent`] inbound (a client called us) and [`Outbound`] back
//! (we must emit a signal to a client). This mirrors the niri event thread.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
};

use iced::widget::image;
use tokio::sync::mpsc;
use zbus::{
    connection,
    fdo::{RequestNameFlags, RequestNameReply},
    object_server::SignalEmitter,
    zvariant::{OwnedValue, Value},
};

use super::model::{NotifAction, NotifImage, NotificationData, Outbound, ServerEvent, Urgency};

const PATH: &str = "/org/freedesktop/Notifications";
const NAME: &str = "org.freedesktop.Notifications";

struct NotificationServer {
    /// Never 0 (0 tells a client "server-assigned"); starts at 1.
    next_id: AtomicU32,
    events: mpsc::Sender<ServerEvent>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        // expire_timeout is ignored on purpose: odin dismisses a toast by an
        // event (user action or window-close), never by a clock.
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        };
        let data = NotificationData {
            id,
            app_name,
            app_icon,
            summary,
            body,
            actions: parse_actions(&actions),
            urgency: parse_urgency(&hints),
            window_id_hint: hint_window_id(&hints, "x-niri-window-id"),
            desktop_entry: hint_str(&hints, "desktop-entry"),
            image: hint_image(&hints),
            suppress_sound: hint_bool(&hints, "suppress-sound"),
            sound_file: hint_str(&hints, "sound-file"),
            category: hint_str(&hints, "category"),
            resident: hint_bool(&hints, "resident"),
        };
        if let Err(e) = self.events.try_send(ServerEvent::Notify(data)) {
            eprintln!("notifications: dropping Notify: {e}");
        }
        id
    }

    async fn close_notification(&self, id: u32) {
        if let Err(e) = self.events.try_send(ServerEvent::CloseRequested(id)) {
            eprintln!("notifications: dropping CloseNotification: {e}");
        }
    }

    fn get_capabilities(&self) -> Vec<String> {
        [
            "body",
            "body-markup",
            "actions",
            "action-icons",
            "icon-static",
            "persistence",
            "inline-reply",
        ]
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "vetrata".to_owned(),
            "wvhulle".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
            "1.2".to_owned(),
        )
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

/// Claim the bus name (stealing it from a running Quickshell odin), register
/// the object server, and spawn the outbound signal drainer. Returns once the
/// name is claimed; the spawned task owns the connection for the app's
/// lifetime.
pub async fn serve(
    events: mpsc::Sender<ServerEvent>,
    mut outbound: mpsc::Receiver<Outbound>,
) -> zbus::Result<()> {
    let server = NotificationServer {
        next_id: AtomicU32::new(1),
        events,
    };
    let conn = connection::Builder::session()?
        .serve_at(PATH, server)?
        .build()
        .await?;

    // ReplaceExisting|AllowReplacement steals the name from odin *if odin itself
    // requested it with AllowReplacement*; otherwise we land InQueue and stay
    // dormant until odin exits.
    match conn
        .request_name_with_flags(
            NAME,
            RequestNameFlags::ReplaceExisting | RequestNameFlags::AllowReplacement,
        )
        .await?
    {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
        RequestNameReply::InQueue | RequestNameReply::Exists => {
            eprintln!(
                "notifications: another daemon owns {NAME} and disallowed replacement; toasts \
                 stay dormant until it exits"
            );
        }
    }

    tokio::spawn(async move {
        let emitter = match SignalEmitter::new(&conn, PATH) {
            Ok(emitter) => emitter,
            Err(e) => {
                eprintln!("notifications: signal emitter: {e}");
                return;
            }
        };
        while let Some(ob) = outbound.recv().await {
            let result = match ob {
                Outbound::ActionInvoked { id, key } => {
                    NotificationServer::action_invoked(&emitter, id, &key).await
                }
                Outbound::Closed { id, reason } => {
                    NotificationServer::notification_closed(&emitter, id, reason).await
                }
            };
            if let Err(e) = result {
                eprintln!("notifications: emit signal: {e}");
            }
        }
        drop(conn);
    });

    Ok(())
}

/// The `--action` list is `[key, label, key, label, ...]`.
fn parse_actions(actions: &[String]) -> Vec<NotifAction> {
    actions
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| NotifAction {
            key: pair[0].clone(),
            label: pair[1].clone(),
        })
        .collect()
}

fn parse_urgency(hints: &HashMap<String, OwnedValue>) -> Urgency {
    match hints
        .get("urgency")
        .and_then(|v| v.downcast_ref::<u8>().ok())
    {
        Some(0) => Urgency::Low,
        Some(2) => Urgency::Critical,
        _ => Urgency::Normal,
    }
}

fn hint_window_id(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<u64> {
    let value = hints.get(key)?;
    u64::try_from(value.downcast_ref::<i32>().ok()?).ok()
}

fn hint_str(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let s = hints.get(key)?.downcast_ref::<&str>().ok()?;
    (!s.is_empty()).then(|| s.to_owned())
}

fn hint_bool(hints: &HashMap<String, OwnedValue>, key: &str) -> bool {
    hints
        .get(key)
        .and_then(|v| v.downcast_ref::<bool>().ok())
        .unwrap_or(false)
}

/// The notification's own image, in freedesktop priority: the raw-pixel
/// `image-data` hint, then the `image-path` name/path, then the deprecated
/// `icon_data` raster.
fn hint_image(hints: &HashMap<String, OwnedValue>) -> Option<NotifImage> {
    if let Some(handle) = ["image-data", "image_data"]
        .iter()
        .find_map(|key| hints.get(*key))
        .and_then(|value| image_data_to_handle(value))
    {
        return Some(NotifImage::Rgba(handle));
    }
    if let Some(path) = hint_str(hints, "image-path").or_else(|| hint_str(hints, "image_path")) {
        return Some(NotifImage::Named(path));
    }
    hints
        .get("icon_data")
        .and_then(|value| image_data_to_handle(value))
        .map(NotifImage::Rgba)
}

/// The `image-data` hint is `(iiibiiay)`: width, height, rowstride, has-alpha,
/// bits-per-sample, channels, and the pixel bytes. iced wants tightly-packed
/// RGBA, so this drops any row padding and widens RGB to RGBA.
fn image_data_to_handle(value: &Value) -> Option<image::Handle> {
    let (width, height, rowstride, has_alpha, _bits, channels, data): (
        i32,
        i32,
        i32,
        bool,
        i32,
        i32,
        Vec<u8>,
    ) = value.try_into().ok()?;
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let rowstride = usize::try_from(rowstride).ok()?;
    let channels = usize::try_from(channels).ok()?;
    if width == 0 || height == 0 || channels < 3 {
        return None;
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        let row = y * rowstride;
        for x in 0..width {
            let pixel = row + x * channels;
            rgba.push(*data.get(pixel)?);
            rgba.push(*data.get(pixel + 1)?);
            rgba.push(*data.get(pixel + 2)?);
            rgba.push(if has_alpha && channels >= 4 {
                *data.get(pixel + 3)?
            } else {
                255
            });
        }
    }
    Some(image::Handle::from_rgba(width as u32, height as u32, rgba))
}
