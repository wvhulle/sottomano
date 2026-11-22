mod notification_hint_parsing;

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
};

use futures::Stream;
use sottomano_compositors::compositor_connection::CompositorKind;
use tokio::sync::{OnceCell, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use zbus::{
    Connection, connection,
    fdo::{DBusProxy, RequestNameFlags, RequestNameReply},
    message::Header,
    names::BusName,
    object_server::SignalEmitter,
    proxy::CacheProperties,
    zvariant::OwnedValue,
};

use self::notification_hint_parsing::{
    hint_progress, hinted_window_id, parse_actions, parse_urgency, reply_prompt,
};
use super::{
    notification_data::{
        DesktopEntryHint, ExpireTimeout, NotifAction, NotificationBody, NotificationData,
        NotifyingAppName, Outbound, SenderWindowHint, ServerEvent,
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
    system_protocol_clients::{
        dbus_desktop_buses::OwnerProcess, procfs_process_ancestry::process_ancestry,
    },
};

const PATH: &str = "/org/freedesktop/Notifications";
const NAME: &str = "org.freedesktop.Notifications";
const INLINE_REPLY: &str = "inline-reply";
const WINDOW_ID_HINT: &str = "x-window-id";
const NIRI_WINDOW_ID_HINT: &str = "x-niri-window-id";

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
            app_name: match non_empty(app_name) {
                Some(named) => NotifyingAppName::Named(named),
                None => NotifyingAppName::Unnamed,
            },
            image,
            image_role,
            app_icon,
            summary,
            body: match non_empty(body) {
                Some(text) => NotificationBody::Text(text),
                None => NotificationBody::Bodiless,
            },
            reply: reply_prompt(reply_actions.first(), &hints),
            actions,
            severity: parse_urgency(&hints),
            expire_timeout: ExpireTimeout::client_requested_millis(expire_timeout),
            window_id_hint: match CompositorKind::detected().and_then(|compositor| {
                hinted_window_id(&hints, |hint_text| {
                    compositor.notification_window_id(hint_text)
                })
            }) {
                Some(window) => SenderWindowHint::Window(window),
                None => SenderWindowHint::Unhinted,
            },
            sender_ancestry: match sender {
                OwnerProcess::Identified(pid) => process_ancestry(pid),
                OwnerProcess::Unidentified => Vec::new(),
            },
            sender,
            desktop_entry: match hint_str(&hints, "desktop-entry") {
                Some(entry) => DesktopEntryHint::Entry(entry),
                None => DesktopEntryHint::Unhinted,
            },
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
