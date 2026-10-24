use std::{
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
use tokio::{
    net::TcpStream,
    time::{sleep, timeout},
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::{secret_file_reads::read_secret, system_protocol_clients::http_json_client};

const CAPABILITIES_PATH: &str = "/ocs/v2.php/cloud/capabilities?format=json";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const FIRST_RETRY: Duration = Duration::from_secs(30);
const LONGEST_RETRY: Duration = Duration::from_mins(30);
const UNSUPPORTED_RECHECK: Duration = Duration::from_hours(1);

type PushSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PushLogin {
    pub server_url: String,
    pub user: String,
    pub password: PathBuf,
}

enum PushLink {
    Connecting { after: Duration },
    Listening(Box<PushSocket>),
}

enum ConnectFailure {
    Unsupported,
    Failed(String),
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
    stream::unfold(
        PushLink::Connecting {
            after: Duration::ZERO,
        },
        move |link| {
            let login = login.clone();
            let push_live = push_live.clone();
            async move { Some(next_activity_ping(link, &login, &push_live).await) }
        },
    )
}

async fn next_activity_ping(
    mut link: PushLink,
    login: &PushLogin,
    push_live: &AtomicBool,
) -> ((), PushLink) {
    loop {
        link = match link {
            PushLink::Connecting { after } => {
                sleep(after).await;
                match listen_for_activity(login).await {
                    Ok(socket) => {
                        push_live.store(true, Ordering::Relaxed);
                        return ((), PushLink::Listening(Box::new(socket)));
                    }
                    Err(ConnectFailure::Unsupported) => PushLink::Connecting {
                        after: UNSUPPORTED_RECHECK,
                    },
                    Err(ConnectFailure::Failed(reason)) => {
                        tracing::warn!("nextcloud notify_push: {reason}");
                        PushLink::Connecting {
                            after: (after * 2).clamp(FIRST_RETRY, LONGEST_RETRY),
                        }
                    }
                }
            }
            PushLink::Listening(mut socket) => match socket.next().await {
                Some(Ok(Message::Text(text))) if text.as_str().starts_with("notify_activity") => {
                    return ((), PushLink::Listening(socket));
                }
                Some(Ok(_)) => PushLink::Listening(socket),
                Some(Err(_)) | None => {
                    push_live.store(false, Ordering::Relaxed);
                    PushLink::Connecting { after: FIRST_RETRY }
                }
            },
        }
    }
}

async fn listen_for_activity(login: &PushLogin) -> Result<PushSocket, ConnectFailure> {
    let password = read_secret(&login.password)
        .ok_or_else(|| ConnectFailure::Failed("password unreadable".to_owned()))?;
    let websocket_url = advertised_websocket(login, &password).await?;
    let (mut socket, _) = connect_async(websocket_url.as_str())
        .await
        .map_err(|error| ConnectFailure::Failed(error.to_string()))?;
    for credential in [login.user.as_str(), password.as_str()] {
        socket
            .send(Message::text(credential))
            .await
            .map_err(|error| ConnectFailure::Failed(error.to_string()))?;
    }
    match timeout(HANDSHAKE_TIMEOUT, socket.next()).await {
        Ok(Some(Ok(Message::Text(reply)))) if reply.as_str() == "authenticated" => Ok(socket),
        Ok(Some(Ok(Message::Text(reply)))) => {
            Err(ConnectFailure::Failed(reply.as_str().to_owned()))
        }
        _ => Err(ConnectFailure::Failed(
            "no authentication reply from notify_push".to_owned(),
        )),
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
        HANDSHAKE_TIMEOUT.as_secs(),
    )
    .await
    .map_err(|error| match error {
        http_json_client::Error::Status { status: 404, .. }
        | http_json_client::Error::Decode(_) => ConnectFailure::Unsupported,
        other => ConnectFailure::Failed(other.to_string()),
    })?;
    capabilities
        .ocs
        .data
        .capabilities
        .notify_push
        .map(|push| push.endpoints.websocket)
        .ok_or(ConnectFailure::Unsupported)
}
