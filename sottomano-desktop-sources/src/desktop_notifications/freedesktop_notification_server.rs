use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
};

use futures::Stream;
use sottomano_toml_config::severity::Severity;
use tokio::sync::{OnceCell, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use uom::si::{f64::Ratio, ratio::percent};
use zbus::{
    Connection, connection,
    fdo::{DBusProxy, RequestNameFlags, RequestNameReply},
    message::Header,
    names::BusName,
    object_server::SignalEmitter,
    proxy::CacheProperties,
    zvariant::OwnedValue,
};

use super::{
    notification_data::{
        ActionTint, ExpireTimeout, NotifAction, NotificationData, Outbound, ReplyPrompt,
        ServerEvent,
    },
    notification_image_policy::image_role,
    spec_hints::{
        HintFlag, hint_flag, hint_str, notification_category, persistence, sound_request,
        spec_app_icon, spec_image,
    },
    vendor_hints::stack_tag,
};
use crate::{
    desktop_notifications::notification_data::Residency,
    non_empty_text::non_empty,
    system_protocol_clients::dbus_desktop_buses::{OwnerProcess, prop},
};

const PATH: &str = "/org/freedesktop/Notifications";
const NAME: &str = "org.freedesktop.Notifications";
const INLINE_REPLY: &str = "inline-reply";

struct NotificationServer {
    next_id: AtomicU32,
    events: mpsc::Sender<ServerEvent>,
    bus_proxy: OnceCell<DBusProxy<'static>>,
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
        let sender = sender_process(&self.bus_proxy, connection, &header).await;
        let id = self.replaced_or_fresh_id(replaces_id);
        let app_icon = spec_app_icon(&app_icon);
        let image = spec_image(&hints, &app_icon);
        let image_role = image_role(&image).await;
        let (reply_actions, actions): (Vec<NotifAction>, Vec<NotifAction>) =
            parse_actions(&actions)
                .into_iter()
                .partition(|action| action.key == INLINE_REPLY);
        let data = NotificationData {
            id,
            app_name: non_empty(app_name),
            image,
            image_role,
            app_icon,
            summary,
            body: non_empty(body),
            reply: reply_prompt(reply_actions.first(), &hints),
            actions,
            severity: parse_urgency(&hints),
            expire_timeout: ExpireTimeout::client_requested_millis(expire_timeout),
            window_id_hint: hint_window_id(&hints, "x-niri-window-id"),
            sender,
            desktop_entry: hint_str(&hints, "desktop-entry"),
            sound_request: sound_request(&hints),
            category: notification_category(&hints),
            residency: match hint_flag(&hints, "resident") {
                HintFlag::Set => Residency::Resident,
                HintFlag::Unset => Residency::Transient,
            },
            persistence: persistence(&hints),
            stack_tag: stack_tag(&hints),
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
            "sound",
            INLINE_REPLY,
        ]
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "sottomano".to_owned(),
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

pub async fn serve() -> zbus::Result<(impl Stream<Item = ServerEvent> + Send, NotificationSignals)>
{
    let (event_sender, event_receiver) = mpsc::channel(256);
    let server = NotificationServer {
        next_id: AtomicU32::new(1),
        events: event_sender,
        bus_proxy: OnceCell::new(),
    };
    let connection = connection::Builder::session()?
        .serve_at(PATH, server)?
        .build()
        .await?;

    match connection
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

    let signals = NotificationSignals {
        signal_emitter: SignalEmitter::new(&connection, PATH)?,
    };
    Ok((ReceiverStream::new(event_receiver), signals))
}

#[derive(Debug, Clone)]
pub struct NotificationSignals {
    signal_emitter: SignalEmitter<'static>,
}

impl NotificationSignals {
    pub async fn emit_outbound_signal(&self, outbound: Outbound) {
        let signal_emitter = &self.signal_emitter;
        let emitted = match outbound {
            Outbound::ActivationToken { id, token } => {
                NotificationServer::activation_token(signal_emitter, id, &token).await
            }
            Outbound::ActionInvoked { id, key } => {
                NotificationServer::action_invoked(signal_emitter, id, &key).await
            }
            Outbound::Replied { id, text } => {
                NotificationServer::notification_replied(signal_emitter, id, &text).await
            }
            Outbound::Closed { id, reason } => {
                NotificationServer::notification_closed(signal_emitter, id, reason).await
            }
        };
        if let Err(emit_failure) = emitted {
            tracing::warn!("notifications: emit signal: {emit_failure}");
        }
    }
}

async fn sender_process(
    bus_proxy: &OnceCell<DBusProxy<'static>>,
    connection: &Connection,
    header: &Header<'_>,
) -> OwnerProcess {
    let Some(sender) = header.sender() else {
        return OwnerProcess::Unidentified;
    };
    let shared_proxy = bus_proxy
        .get_or_try_init(|| {
            DBusProxy::builder(connection)
                .cache_properties(CacheProperties::No)
                .build()
        })
        .await;
    match shared_proxy {
        Ok(shared_bus_proxy) => {
            OwnerProcess::query_bus_owner(shared_bus_proxy, BusName::from(sender.clone())).await
        }
        Err(proxy_failure) => {
            tracing::debug!(error = %proxy_failure, "notifications: bus proxy is unavailable");
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
            tint: ActionTint::from(pair[0].as_str()),
        })
        .collect()
}

fn parse_urgency(hints: &HashMap<String, OwnedValue>) -> Severity {
    match prop::<u8>(hints, "urgency") {
        Some(0) => Severity::Low,
        Some(2) => Severity::Critical,
        _ => Severity::Normal,
    }
}

fn hint_window_id(hints: &HashMap<String, OwnedValue>, key: &str) -> Option<u64> {
    let signed_window_id = prop::<i32>(hints, key)?;
    u64::try_from(signed_window_id)
        .inspect_err(|_| {
            tracing::warn!("notifications: hint {key} has negative window id {signed_window_id}");
        })
        .ok()
}

fn hint_progress(hints: &HashMap<String, OwnedValue>) -> Option<Ratio> {
    prop::<i32>(hints, "value").map(clamped_progress)
}

fn clamped_progress(percent_value: i32) -> Ratio {
    Ratio::new::<percent>(f64::from(percent_value.clamp(0, 100)))
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uom::si::{f64::Ratio, ratio::percent};
    use zbus::zvariant::{OwnedValue, Str};

    use super::{ActionTint, NotifAction, ReplyPrompt, clamped_progress, reply_prompt};

    #[test]
    fn progress_hint_is_clamped_to_percent_range() {
        let percent_of = |value| Ratio::new::<percent>(value);
        assert_eq!(clamped_progress(-5), percent_of(0.0));
        assert_eq!(clamped_progress(42), percent_of(42.0));
        assert_eq!(clamped_progress(250), percent_of(100.0));
    }

    fn reply_action() -> NotifAction {
        NotifAction {
            key: "inline-reply".to_owned(),
            label: "Reply".to_owned(),
            tint: ActionTint::Proceed,
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
