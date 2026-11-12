use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    future::ready,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use futures::{SinkExt, Stream, StreamExt, stream};
use serde::Deserialize;
use tokio::{net::TcpStream, time::timeout};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, Message},
};

use crate::{
    secret_file_reads::read_secret,
    system_protocol_clients::{http_json_client, protocol_peer_reconnects::subscribed},
};

const CAPABILITIES_PATH: &str = "/ocs/v2.php/cloud/capabilities?format=json";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

type PushSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PushLogin {
    pub server_url: String,
    pub user: String,
    pub password: PathBuf,
}

#[derive(Clone)]
struct PushSubscription {
    login: PushLogin,
    push_live: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum NotifyPushFailure {
    Unsupported,
    PasswordUnreadable,
    Capabilities(http_json_client::Error),
    WebSocket(WebSocketError),
    AuthenticationRefused,
    NoAuthenticationReply,
}

impl Display for NotifyPushFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            NotifyPushFailure::Unsupported => write!(f, "server offers no notify_push"),
            NotifyPushFailure::PasswordUnreadable => write!(f, "password unreadable"),
            NotifyPushFailure::Capabilities(error) => write!(f, "capabilities request: {error}"),
            NotifyPushFailure::WebSocket(error) => write!(f, "websocket: {error}"),
            NotifyPushFailure::AuthenticationRefused => write!(f, "credentials refused"),
            NotifyPushFailure::NoAuthenticationReply => write!(f, "no authentication reply"),
        }
    }
}

impl From<WebSocketError> for NotifyPushFailure {
    fn from(error: WebSocketError) -> Self {
        NotifyPushFailure::WebSocket(error)
    }
}

impl From<http_json_client::Error> for NotifyPushFailure {
    fn from(error: http_json_client::Error) -> Self {
        match error {
            http_json_client::Error::Status { status: 404, .. }
            | http_json_client::Error::Decode(_) => NotifyPushFailure::Unsupported,
            other => NotifyPushFailure::Capabilities(other),
        }
    }
}

#[derive(Deserialize)]
struct CapabilitiesEnvelope {
    ocs: OcsPayload,
}

#[derive(Deserialize)]
struct OcsPayload {
    data: OcsData,
}

#[derive(Deserialize)]
struct OcsData {
    capabilities: Capabilities,
}

#[derive(Deserialize)]
struct Capabilities {
    notify_push: Option<NotifyPushCapability>,
}

#[derive(Deserialize)]
struct NotifyPushCapability {
    endpoints: NotifyPushEndpoints,
}

#[derive(Deserialize)]
struct NotifyPushEndpoints {
    websocket: String,
}

pub fn activity_pings(
    login: PushLogin,
    push_live: Arc<AtomicBool>,
) -> impl Stream<Item = Result<(), NotifyPushFailure>> + Send {
    subscribed(
        PushSubscription { login, push_live },
        |push_subscription| async move {
            let socket = match listen_for_activity(&push_subscription.login).await {
                Ok(socket) => socket,
                Err(NotifyPushFailure::Unsupported) => {
                    return Ok(stream::pending().boxed());
                }
                Err(connect_failure) => return Err(connect_failure),
            };
            push_subscription.push_live.store(true, Ordering::Relaxed);
            let lost_link = push_subscription.push_live.clone();
            Ok(socket
                .filter_map(|message| {
                    ready(match message {
                        Ok(Message::Text(text)) if text.as_str().starts_with("notify_activity") => {
                            Some(Ok(()))
                        }
                        Ok(_) => None,
                        Err(socket_failure) => Some(Err(NotifyPushFailure::from(socket_failure))),
                    })
                })
                .chain(
                    stream::once(async move {
                        lost_link.store(false, Ordering::Relaxed);
                    })
                    .filter_map(|()| ready(None)),
                )
                .boxed())
        },
    )
}

async fn listen_for_activity(login: &PushLogin) -> Result<PushSocket, NotifyPushFailure> {
    let Some(password) = read_secret(&login.password) else {
        return Err(NotifyPushFailure::PasswordUnreadable);
    };
    let websocket_url = advertised_websocket(login, &password).await?;
    let (mut socket, _) = connect_async(websocket_url.as_str()).await?;
    socket.send(Message::text(login.user.as_str())).await?;
    socket.send(Message::text(password.as_str())).await?;
    match timeout(HANDSHAKE_TIMEOUT, socket.next()).await {
        Ok(Some(Ok(Message::Text(reply)))) if reply.as_str() == "authenticated" => Ok(socket),
        Ok(Some(Ok(Message::Text(_)))) => Err(NotifyPushFailure::AuthenticationRefused),
        _ => Err(NotifyPushFailure::NoAuthenticationReply),
    }
}

async fn advertised_websocket(
    login: &PushLogin,
    password: &str,
) -> Result<String, NotifyPushFailure> {
    let url = format!(
        "{}{CAPABILITIES_PATH}",
        login.server_url.trim_end_matches('/')
    );
    let authorization = format!(
        "Basic {}",
        STANDARD.encode(format!("{}:{password}", login.user))
    );
    let capabilities = http_json_client::get_json::<CapabilitiesEnvelope>(
        &url,
        &[
            ("OCS-APIREQUEST", "true"),
            ("Accept", "application/json"),
            ("Authorization", &authorization),
        ],
    )
    .await?;
    let Some(notify_push) = capabilities.ocs.data.capabilities.notify_push else {
        return Err(NotifyPushFailure::Unsupported);
    };
    Ok(notify_push.endpoints.websocket)
}
