use std::{
    env,
    io::{self, BufRead, BufReader, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::PathBuf,
};

const HYPRLAND_SIGNATURE_ENV: &str = "HYPRLAND_INSTANCE_SIGNATURE";
const HYPRLAND_REQUEST_SOCKET: &str = ".socket.sock";
const HYPRLAND_EVENT_SOCKET: &str = ".socket2.sock";
const HYPRLAND_EVENT_SEPARATOR: &str = ">>";

#[must_use]
pub fn hyprland_signature_is_set() -> bool {
    env::var_os(HYPRLAND_SIGNATURE_ENV).is_some()
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

pub fn hyprland_request(command: &str) -> io::Result<String> {
    let mut stream =
        UnixStream::connect(hyprland_socket_directory()?.join(HYPRLAND_REQUEST_SOCKET))?;
    stream.write_all(command.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

pub struct HyprlandEvent {
    pub name: String,
    pub data: String,
}

pub struct HyprlandEventSubscription {
    lines: BufReader<UnixStream>,
}

impl HyprlandEventSubscription {
    pub fn connect() -> io::Result<Self> {
        let stream = UnixStream::connect(hyprland_socket_directory()?.join(HYPRLAND_EVENT_SOCKET))?;
        Ok(Self {
            lines: BufReader::new(stream),
        })
    }

    pub fn next_event(&mut self) -> io::Result<HyprlandEvent> {
        let mut line = String::new();
        if self.lines.read_line(&mut line)? == 0 {
            return Err(io::Error::other("Hyprland closed its event socket"));
        }
        let line = line.trim_end();
        let (name, data) = line
            .split_once(HYPRLAND_EVENT_SEPARATOR)
            .unwrap_or((line, ""));
        Ok(HyprlandEvent {
            name: name.to_owned(),
            data: data.to_owned(),
        })
    }
}
