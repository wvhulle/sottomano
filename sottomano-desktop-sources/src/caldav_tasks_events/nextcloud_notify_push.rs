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

enum ConnectFailure {
    Unsupported,
    PasswordUnreadable,
    Capabilities(http_json_client::Error),
    WebSocket(WebSocketError),
    AuthenticationRefused,
    NoAuthenticationReply,
}

impl Display for ConnectFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ConnectFailure::Unsupported => write!(f, "server offers no notify_push"),
            ConnectFailure::PasswordUnreadable => write!(f, "password unreadable"),
            ConnectFailure::Capabilities(error) => write!(f, "capabilities request: {error}"),
            ConnectFailure::WebSocket(error) => write!(f, "websocket: {error}"),
            ConnectFailure::AuthenticationRefused => write!(f, "credentials refused"),
            ConnectFailure::NoAuthenticationReply => write!(f, "no authentication reply"),
        }
    }
}

impl From<WebSocketError> for ConnectFailure {
    fn from(error: WebSocketError) -> Self {
        ConnectFailure::WebSocket(error)
    }
}

impl From<http_json_client::Error> for ConnectFailure {
    fn from(error: http_json_client::Error) -> Self {
        match error {
            http_json_client::Error::Status { status: 404, .. }
            | http_json_client::Error::Decode(_) => ConnectFailure::Unsupported,
            other => ConnectFailure::Capabilities(other),
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
) -> impl Stream<Item = ()> + Send {
    subscribed(
        PushSubscription { login, push_live },
        |push_subscription| async move {
            let socket = listen_for_activity(&push_subscription.login).await?;
            push_subscription.push_live.store(true, Ordering::Relaxed);
            let lost_link = push_subscription.push_live.clone();
            Ok(socket
                .filter_map(|message| {
                    ready(match message {
                        Ok(Message::Text(text)) if text.as_str().starts_with("notify_activity") => {
                            Some(Ok::<(), ConnectFailure>(()))
                        }
                        Ok(_) | Err(_) => None,
                    })
                })
                .chain(
                    stream::once(async move {
                        lost_link.store(false, Ordering::Relaxed);
                    })
                    .filter_map(|()| ready(None)),
                ))
        },
    )
    .filter_map(|activity_ping| {
        ready(
            activity_ping
                .inspect_err(|failure| tracing::warn!("nextcloud notify_push: {failure}"))
                .ok(),
        )
    })
}

async fn listen_for_activity(login: &PushLogin) -> Result<PushSocket, ConnectFailure> {
    let Some(password) = read_secret(&login.password) else {
        return Err(ConnectFailure::PasswordUnreadable);
    };
    let websocket_url = advertised_websocket(login, &password).await?;
    let (mut socket, _) = connect_async(websocket_url.as_str()).await?;
    socket.send(Message::text(login.user.as_str())).await?;
    socket.send(Message::text(password.as_str())).await?;
    match timeout(HANDSHAKE_TIMEOUT, socket.next()).await {
        Ok(Some(Ok(Message::Text(reply)))) if reply.as_str() == "authenticated" => Ok(socket),
        Ok(Some(Ok(Message::Text(_)))) => Err(ConnectFailure::AuthenticationRefused),
        _ => Err(ConnectFailure::NoAuthenticationReply),
    }
}

async fn advertised_websocket(login: &PushLogin, password: &str) -> Result<String, ConnectFailure> {
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
        return Err(ConnectFailure::Unsupported);
    };
    Ok(notify_push.endpoints.websocket)
}
