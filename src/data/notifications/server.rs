use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
};

use iced::widget::image;
use tokio::sync::mpsc;
use zbus::{
    Connection, connection,
    fdo::{DBusProxy, RequestNameFlags, RequestNameReply},
    message::Header,
    names::BusName,
    object_server::SignalEmitter,
    proxy::CacheProperties,
    zvariant::{OwnedValue, Value},
};

use super::model::{
    ExpireTimeout, NotifAction, NotifImage, NotificationData, Outbound, Progress, ReplyPrompt,
    ServerEvent,
};
use crate::data::{empty_text::non_empty, severity::Severity, source::bus::OwnerProcess};

const PATH: &str = "/org/freedesktop/Notifications";
const NAME: &str = "org.freedesktop.Notifications";
const INLINE_REPLY: &str = "inline-reply";

struct NotificationServer {
    next_id: AtomicU32,
    events: mpsc::Sender<ServerEvent>,
}

impl NotificationServer {
    fn replaced_or_fresh_id(&self, replaces_id: u32) -> u32 {
        let issued_by_this_server =
            replaces_id != 0 && replaces_id < self.next_id.load(Ordering::Relaxed);
        if issued_by_this_server {
            replaces_id
        } else {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        }
    }
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
#[allow(clippy::unused_async_trait_impl, clippy::unused_self)]
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
        expire_timeout: i32,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> u32 {
        let sender = sender_process(connection, &header).await;
        let id = self.replaced_or_fresh_id(replaces_id);
        let (reply_actions, actions): (Vec<NotifAction>, Vec<NotifAction>) =
            parse_actions(&actions)
                .into_iter()
                .partition(|action| action.key == INLINE_REPLY);
        let data = NotificationData {
            id,
            app_name: non_empty(app_name),
            app_icon: non_empty(app_icon),
            summary,
            body: non_empty(body),
            reply: reply_prompt(reply_actions.first(), &hints),
            actions,
            severity: parse_urgency(&hints),
            expire_timeout: ExpireTimeout::from_millis(expire_timeout),
            window_id_hint: hint_window_id(&hints, "x-niri-window-id"),
            sender,
            desktop_entry: hint_str(&hints, "desktop-entry"),
            image: hint_image(&hints),
            suppress_sound: hint_bool(&hints, "suppress-sound"),
            sound_file: hint_str(&hints, "sound-file"),
            category: hint_str(&hints, "category"),
            resident: hint_bool(&hints, "resident"),
            progress: hint_progress(&hints),
        };
        if let Err(e) = self.events.send(ServerEvent::Notify(Box::new(data))).await {
            tracing::error!("notifications: dropping Notify: {e}");
        }
        id
    }

    async fn close_notification(&self, id: u32) {
        if let Err(e) = self.events.send(ServerEvent::CloseRequested(id)).await {
            tracing::error!("notifications: dropping CloseNotification: {e}");
        }
    }

    fn get_capabilities(&self) -> Vec<String> {
        [
            "body",
            "body-markup",
            "actions",
            "icon-static",
            "persistence",
            INLINE_REPLY,
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
    async fn notification_replied(
        emitter: &SignalEmitter<'_>,
        id: u32,
        text: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn activation_token(
        emitter: &SignalEmitter<'_>,
        id: u32,
        activation_token: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn action_invoked(
        emitter: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

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

    match conn
        .request_name_with_flags(
            NAME,
            RequestNameFlags::ReplaceExisting | RequestNameFlags::AllowReplacement,
        )
        .await?
    {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
        RequestNameReply::InQueue | RequestNameReply::Exists => {
            tracing::error!(
                "notifications: another daemon owns {NAME} and disallowed replacement; toasts \
                 stay dormant until it exits"
            );
        }
    }

    tokio::spawn(async move {
        let emitter = match SignalEmitter::new(&conn, PATH) {
            Ok(emitter) => emitter,
            Err(e) => {
                tracing::error!("notifications: signal emitter: {e}");
                return;
            }
        };
        while let Some(ob) = outbound.recv().await {
            let result = match ob {
                Outbound::ActivationToken { id, token } => {
                    NotificationServer::activation_token(&emitter, id, &token).await
                }
                Outbound::ActionInvoked { id, key } => {
                    NotificationServer::action_invoked(&emitter, id, &key).await
                }
                Outbound::Replied { id, text } => {
                    NotificationServer::notification_replied(&emitter, id, &text).await
                }
                Outbound::Closed { id, reason } => {
                    NotificationServer::notification_closed(&emitter, id, reason).await
                }
            };
            if let Err(e) = result {
                tracing::warn!("notifications: emit signal: {e}");
            }
        }
        drop(conn);
    });

    Ok(())
}

async fn sender_process(connection: &Connection, header: &Header<'_>) -> OwnerProcess {
    let Some(sender) = header.sender() else {
        return OwnerProcess::Unidentified;
    };
    let dbus = DBusProxy::builder(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await;
    match dbus {
        Ok(dbus) => OwnerProcess::of_bus(&dbus, BusName::from(sender.clone())).await,
        Err(e) => {
            tracing::debug!("notifications: bus proxy: {e}");
            OwnerProcess::Unidentified
        }
    }
}

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

fn parse_urgency(hints: &HashMap<String, OwnedValue>) -> Severity {
    match hints
        .get("urgency")
        .and_then(|v| v.downcast_ref::<u8>().ok())
    {
        Some(0) => Severity::Low,
        Some(2) => Severity::Critical,
        _ => Severity::Normal,
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

fn hint_progress(hints: &HashMap<String, OwnedValue>) -> Progress {
    hints
        .get("value")
        .and_then(|v| v.downcast_ref::<i32>().ok())
        .map_or(Progress::Absent, Progress::from_hint)
}

fn reply_prompt(action: Option<&NotifAction>, hints: &HashMap<String, OwnedValue>) -> ReplyPrompt {
    let Some(action) = action else {
        return ReplyPrompt::NotOffered;
    };
    ReplyPrompt::Offered {
        placeholder: hint_str(hints, "x-kde-reply-placeholder-text")
            .unwrap_or_else(|| action.label.clone()),
        submit: hint_str(hints, "x-kde-reply-submit-button-text")
            .unwrap_or_else(|| "Send".to_owned()),
    }
}

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
    Some(image::Handle::from_rgba(
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
        rgba,
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zbus::zvariant::{OwnedValue, Str};

    use super::{NotifAction, ReplyPrompt, reply_prompt};

    fn reply_action() -> NotifAction {
        NotifAction {
            key: "inline-reply".to_owned(),
            label: "Reply".to_owned(),
        }
    }

    fn text(value: &str) -> OwnedValue {
        OwnedValue::from(Str::from(value.to_owned()))
    }

    #[test]
    fn reply_without_inline_reply_action_is_not_offered() {
        assert_eq!(reply_prompt(None, &HashMap::new()), ReplyPrompt::NotOffered);
    }

    #[test]
    fn reply_falls_back_to_action_label_and_send() {
        assert_eq!(
            reply_prompt(Some(&reply_action()), &HashMap::new()),
            ReplyPrompt::Offered {
                placeholder: "Reply".to_owned(),
                submit: "Send".to_owned(),
            }
        );
    }

    #[test]
    fn reply_takes_kde_hint_texts() {
        let hints = HashMap::from([
            (
                "x-kde-reply-placeholder-text".to_owned(),
                text("Type an SMS"),
            ),
            (
                "x-kde-reply-submit-button-text".to_owned(),
                text("Send SMS"),
            ),
        ]);
        assert_eq!(
            reply_prompt(Some(&reply_action()), &hints),
            ReplyPrompt::Offered {
                placeholder: "Type an SMS".to_owned(),
                submit: "Send SMS".to_owned(),
            }
        );
    }
}
