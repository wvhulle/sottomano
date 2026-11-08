use std::{env, ffi::OsString};

use serde::de::DeserializeOwned;
use swayipc_types::{CommandType, Error, Event, EventType, Fallible, MAGIC, Success};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

const SWAY_SOCKET_ENV: &str = "SWAYSOCK";
const SWAY_HEADER_LENGTH: usize = 14;

#[must_use]
pub fn sway_socket_variable() -> Option<OsString> {
    env::var_os(SWAY_SOCKET_ENV)
}

pub struct SwayIpcConnection {
    stream: UnixStream,
}

impl SwayIpcConnection {
    pub async fn connect() -> Fallible<Self> {
        let socket_path = env::var_os(SWAY_SOCKET_ENV).ok_or(Error::SocketNotFound)?;
        Ok(Self {
            stream: UnixStream::connect(socket_path).await?,
        })
    }

    pub async fn request<Reply: DeserializeOwned>(
        &mut self,
        command: CommandType,
        payload: &str,
    ) -> Fallible<Reply> {
        self.stream.write_all(&command.encode_with(payload)).await?;
        command.decode(self.read_message().await?)
    }

    pub async fn subscribe(mut self, events: &[EventType]) -> Fallible<SwayEventSubscription> {
        let event_names = serde_json::to_string(events)?;
        let subscribed: Success = self.request(CommandType::Subscribe, &event_names).await?;
        if !subscribed.success {
            return Err(Error::SubscriptionFailed(event_names));
        }
        Ok(SwayEventSubscription { connection: self })
    }

    async fn read_message(&mut self) -> Fallible<(u32, Vec<u8>)> {
        let mut header = [0; SWAY_HEADER_LENGTH];
        self.stream.read_exact(&mut header).await?;
        let [
            magic_0,
            magic_1,
            magic_2,
            magic_3,
            magic_4,
            magic_5,
            length_0,
            length_1,
            length_2,
            length_3,
            type_0,
            type_1,
            type_2,
            type_3,
        ] = header;
        let received_magic = [magic_0, magic_1, magic_2, magic_3, magic_4, magic_5];
        if received_magic != MAGIC {
            return Err(Error::InvalidMagic(received_magic));
        }
        let payload_length = u32::from_ne_bytes([length_0, length_1, length_2, length_3]);
        let message_type = u32::from_ne_bytes([type_0, type_1, type_2, type_3]);
        let mut payload = vec![0; usize::try_from(payload_length).unwrap_or(usize::MAX)];
        self.stream.read_exact(&mut payload).await?;
        Ok((message_type, payload))
    }
}

pub struct SwayEventSubscription {
    connection: SwayIpcConnection,
}

impl SwayEventSubscription {
    pub async fn next_event(&mut self) -> Fallible<Event> {
        Event::decode(self.connection.read_message().await?)
    }
}
