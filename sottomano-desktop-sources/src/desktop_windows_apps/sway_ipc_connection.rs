use std::{
    env,
    ffi::OsString,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
};

use serde::Deserialize;

const SWAY_SOCKET_ENV: &str = "SWAYSOCK";
const SWAY_IPC_MAGIC: &[u8; 6] = b"i3-ipc";
const SWAY_HEADER_LENGTH: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwayMessageKind {
    RunCommand,
    GetWorkspaces,
    Subscribe,
    GetOutputs,
    GetTree,
    GetVersion,
}

impl SwayMessageKind {
    const fn code(self) -> u32 {
        match self {
            SwayMessageKind::RunCommand => 0,
            SwayMessageKind::GetWorkspaces => 1,
            SwayMessageKind::Subscribe => 2,
            SwayMessageKind::GetOutputs => 3,
            SwayMessageKind::GetTree => 4,
            SwayMessageKind::GetVersion => 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwayEventKind {
    Workspace,
    Output,
    Window,
    Shutdown,
    Other,
}

impl From<u32> for SwayEventKind {
    fn from(code: u32) -> Self {
        match code & !0x8000_0000 {
            0 => SwayEventKind::Workspace,
            1 => SwayEventKind::Output,
            3 => SwayEventKind::Window,
            6 => SwayEventKind::Shutdown,
            _ => SwayEventKind::Other,
        }
    }
}

#[derive(Deserialize)]
struct SwaySubscribeReply {
    success: bool,
}

pub struct SwayEvent {
    pub kind: SwayEventKind,
    pub payload: String,
}

pub struct SwayIpcConnection {
    stream: UnixStream,
}

#[must_use]
pub fn sway_socket_variable() -> Option<OsString> {
    env::var_os(SWAY_SOCKET_ENV)
}

impl SwayIpcConnection {
    pub fn connect() -> io::Result<Self> {
        let socket_path = env::var_os(SWAY_SOCKET_ENV)
            .ok_or_else(|| io::Error::other(format!("{SWAY_SOCKET_ENV} is not set")))?;
        Ok(Self {
            stream: UnixStream::connect(socket_path)?,
        })
    }

    pub fn request(&mut self, kind: SwayMessageKind, payload: &str) -> io::Result<String> {
        self.send(kind, payload)?;
        let (_, reply) = self.read_message()?;
        Ok(reply)
    }

    pub fn subscribe(mut self, event_names: &[&str]) -> io::Result<SwayEventSubscription> {
        let payload = serde_json::to_string(event_names)?;
        let reply = self.request(SwayMessageKind::Subscribe, &payload)?;
        let subscribed: SwaySubscribeReply = serde_json::from_str(&reply)?;
        if !subscribed.success {
            return Err(io::Error::other(format!(
                "sway refused the subscription: {reply}"
            )));
        }
        Ok(SwayEventSubscription { connection: self })
    }

    fn send(&mut self, kind: SwayMessageKind, payload: &str) -> io::Result<()> {
        let payload_length = u32::try_from(payload.len())
            .map_err(|_| io::Error::other("sway message payload is too long"))?;
        let mut message = Vec::with_capacity(SWAY_HEADER_LENGTH + payload.len());
        message.extend_from_slice(SWAY_IPC_MAGIC);
        message.extend_from_slice(&payload_length.to_ne_bytes());
        message.extend_from_slice(&kind.code().to_ne_bytes());
        message.extend_from_slice(payload.as_bytes());
        self.stream.write_all(&message)
    }

    fn read_message(&mut self) -> io::Result<(u32, String)> {
        let mut header = [0; SWAY_HEADER_LENGTH];
        self.stream.read_exact(&mut header)?;
        if &header[..6] != SWAY_IPC_MAGIC {
            return Err(io::Error::other(
                "sway sent a message without the i3-ipc magic",
            ));
        }
        let payload_length = u32::from_ne_bytes([header[6], header[7], header[8], header[9]]);
        let code = u32::from_ne_bytes([header[10], header[11], header[12], header[13]]);
        let mut payload = vec![0; usize::try_from(payload_length).map_err(io::Error::other)?];
        self.stream.read_exact(&mut payload)?;
        let text = String::from_utf8(payload).map_err(io::Error::other)?;
        Ok((code, text))
    }
}

pub struct SwayEventSubscription {
    connection: SwayIpcConnection,
}

impl SwayEventSubscription {
    pub fn next_event(&mut self) -> io::Result<SwayEvent> {
        let (code, payload) = self.connection.read_message()?;
        Ok(SwayEvent {
            kind: SwayEventKind::from(code),
            payload,
        })
    }
}
