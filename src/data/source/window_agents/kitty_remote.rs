use std::{
    ffi::OsStr,
    io::{self, Read, Write},
    os::unix::{ffi::OsStrExt, net::UnixStream},
    path::{Path, PathBuf},
};

use procfs::process::Process as Proc;
use serde::{Deserialize, Serialize};

use super::AgentWindow;

const FRAME_START: &[u8] = b"\x1bP@kitty-cmd";
const FRAME_END: &[u8] = b"\x1b\\";
const PROTOCOL_VERSION: [u32; 3] = [0, 48, 2];
const LISTEN_ON_VARIABLE: &str = "KITTY_LISTEN_ON";
const KITTY_PID_VARIABLE: &str = "KITTY_PID";
const UNIX_SCHEME: &[u8] = b"unix:";

pub fn socket_of(kitty_pid: i32) -> Option<PathBuf> {
    let kitty = Proc::new(kitty_pid).ok()?;
    let children = kitty
        .tasks()
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|task| task.children().ok())
        .flatten();
    children
        .filter_map(|child| Proc::new(i32::try_from(child).ok()?).ok()?.environ().ok())
        .find_map(|environment| {
            let spawned_by = environment.get(OsStr::new(KITTY_PID_VARIABLE))?;
            if spawned_by.as_bytes() != kitty_pid.to_string().as_bytes() {
                return None;
            }
            let listen_on = environment.get(OsStr::new(LISTEN_ON_VARIABLE))?;
            let path = listen_on.as_bytes().strip_prefix(UNIX_SCHEME)?;
            Some(PathBuf::from(OsStr::from_bytes(path)))
        })
}

pub fn list(socket: &Path) -> io::Result<Vec<AgentWindow>> {
    let reply = exchange(socket, "ls", NoPayload {})?;
    let listing = reply
        .data
        .ok_or_else(|| io::Error::other("kitty ls replied without data"))?;
    let os_windows: Vec<OsWindow> = serde_json::from_str(&listing)?;
    Ok(os_windows
        .into_iter()
        .filter_map(OsWindow::agent_window)
        .collect())
}

pub fn hold_title(socket: &Path, window: &AgentWindow, title: &str) -> io::Result<()> {
    retitle(socket, window, title, false)
}

pub fn yield_title(socket: &Path, window: &AgentWindow, title: &str) -> io::Result<()> {
    retitle(socket, window, title, true)
}

fn retitle(socket: &Path, window: &AgentWindow, title: &str, temporary: bool) -> io::Result<()> {
    exchange(
        socket,
        "set-window-title",
        TitlePayload {
            title,
            r#match: format!("id:{}", window.title_holder),
            temporary,
        },
    )
    .map(drop)
}

#[derive(Serialize)]
struct NoPayload {}

#[derive(Serialize)]
struct TitlePayload<'a> {
    title: &'a str,
    r#match: String,
    temporary: bool,
}

#[derive(Serialize)]
struct Command<'a, P> {
    cmd: &'a str,
    version: [u32; 3],
    no_response: bool,
    payload: P,
}

#[derive(Deserialize)]
struct Reply {
    ok: bool,
    data: Option<String>,
    error: Option<String>,
}

fn exchange(socket: &Path, cmd: &str, payload: impl Serialize) -> io::Result<Reply> {
    let mut stream = UnixStream::connect(socket)?;
    let command = Command {
        cmd,
        version: PROTOCOL_VERSION,
        no_response: false,
        payload,
    };
    let mut frame = FRAME_START.to_vec();
    frame.extend(serde_json::to_vec(&command)?);
    frame.extend(FRAME_END);
    stream.write_all(&frame)?;
    let mut received = Vec::new();
    let mut chunk = [0; 16 * 1024];
    while !received.ends_with(FRAME_END) {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        received.extend_from_slice(&chunk[..read]);
    }
    let body = received
        .strip_prefix(FRAME_START)
        .and_then(|framed| framed.strip_suffix(FRAME_END))
        .ok_or_else(|| io::Error::other("kitty reply is not a kitty-cmd frame"))?;
    let reply: Reply = serde_json::from_slice(body)?;
    if reply.ok {
        Ok(reply)
    } else {
        Err(io::Error::other(reply.error.unwrap_or_default()))
    }
}

#[derive(Deserialize)]
struct OsWindow {
    id: u64,
    tabs: Vec<Tab>,
}

#[derive(Deserialize)]
struct Tab {
    is_active: bool,
    windows: Vec<KittyWindow>,
}

#[derive(Deserialize)]
struct KittyWindow {
    id: u64,
    pid: i32,
    title: String,
    is_active: bool,
}

impl OsWindow {
    fn agent_window(self) -> Option<AgentWindow> {
        let shown = self
            .tabs
            .iter()
            .find(|tab| tab.is_active)?
            .windows
            .iter()
            .find(|window| window.is_active)?;
        Some(AgentWindow {
            id: self.id,
            title_holder: shown.id,
            title: shown.title.clone(),
            pids: self
                .tabs
                .iter()
                .flat_map(|tab| tab.windows.iter().map(|window| window.pid))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::OsWindow;

    const LISTING: &str = r#"[
      {"id": 92, "is_focused": false, "platform_window_id": null, "tabs": [
        {"id": 1, "is_active": false, "windows": [
          {"id": 7, "pid": 400, "title": "background tab", "is_active": true}
        ]},
        {"id": 2, "is_active": true, "windows": [
          {"id": 93, "pid": 500, "title": "vetrata — pkill", "is_active": true},
          {"id": 94, "pid": 501, "title": "split", "is_active": false}
        ]}
      ]}
    ]"#;

    #[test]
    fn an_os_window_is_titled_by_its_active_tab_and_owns_every_shell() {
        let os_windows: Vec<OsWindow> = serde_json::from_str(LISTING).unwrap();
        let window = os_windows
            .into_iter()
            .next()
            .unwrap()
            .agent_window()
            .unwrap();
        assert_eq!(window.id, 92);
        assert_eq!(window.title_holder, 93);
        assert_eq!(window.title, "vetrata — pkill");
        assert_eq!(window.pids, [400, 500, 501]);
    }
}
