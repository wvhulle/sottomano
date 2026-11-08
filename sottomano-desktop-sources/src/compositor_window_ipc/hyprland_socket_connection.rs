use std::{env, ffi::OsString, io, path::PathBuf};

use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    net::UnixStream,
};

const HYPRLAND_SIGNATURE_ENV: &str = "HYPRLAND_INSTANCE_SIGNATURE";
const HYPRLAND_REQUEST_SOCKET: &str = ".socket.sock";
const HYPRLAND_EVENT_SOCKET: &str = ".socket2.sock";
const HYPRLAND_EVENT_SEPARATOR: &str = ">>";

#[must_use]
pub fn hyprland_signature_variable() -> Option<OsString> {
    env::var_os(HYPRLAND_SIGNATURE_ENV)
}

fn hyprland_socket_directory() -> io::Result<PathBuf> {
    let signature = env::var_os(HYPRLAND_SIGNATURE_ENV)
        .ok_or_else(|| io::Error::other(format!("{HYPRLAND_SIGNATURE_ENV} is not set")))?;
    let runtime_directory = env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| io::Error::other("XDG_RUNTIME_DIR is not set"))?;
    Ok(PathBuf::from(runtime_directory)
        .join("hypr")
        .join(signature))
}

pub async fn hyprland_request(command: &str) -> io::Result<String> {
    let mut stream =
        UnixStream::connect(hyprland_socket_directory()?.join(HYPRLAND_REQUEST_SOCKET)).await?;
    stream.write_all(command.as_bytes()).await?;
    stream.shutdown().await?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply).await?;
    Ok(reply)
}

pub struct HyprlandEventSubscription {
    lines: Lines<BufReader<UnixStream>>,
}

impl HyprlandEventSubscription {
    pub async fn connect() -> io::Result<Self> {
        let stream =
            UnixStream::connect(hyprland_socket_directory()?.join(HYPRLAND_EVENT_SOCKET)).await?;
        Ok(Self {
            lines: BufReader::new(stream).lines(),
        })
    }

    pub async fn next_event_name(&mut self) -> io::Result<Option<String>> {
        let Some(line) = self.lines.next_line().await? else {
            return Ok(None);
        };
        let event_name = line
            .split_once(HYPRLAND_EVENT_SEPARATOR)
            .map_or(line.as_str(), |(event_name, _)| event_name);
        Ok(Some(event_name.to_owned()))
    }
}
