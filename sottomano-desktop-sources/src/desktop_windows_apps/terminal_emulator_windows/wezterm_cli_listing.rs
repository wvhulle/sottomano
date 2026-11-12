use std::{
    collections::BTreeMap, ffi::OsStr, io, os::unix::ffi::OsStrExt, path::PathBuf, process::Command,
};

use procfs::process::Process as Proc;
use serde::Deserialize;

use super::{
    TerminalLabelReading, TerminalWindow, TerminalWindowKey, TitleProbeTarget,
    pty_shell_sessions::{child_pids, shell_sessions},
};

const SOCKET_VARIABLE: &str = "WEZTERM_UNIX_SOCKET";
const CLI_BINARY: &str = "wezterm";

#[derive(Debug, Deserialize)]
struct WeztermPane {
    window_id: u64,
    title: String,
    window_title: String,
    is_active: bool,
    tty_name: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShownWeztermPane {
    wezterm_window: u64,
    pty_path: PathBuf,
    title: String,
    window_ptys: Vec<PathBuf>,
}

fn shown_panes(panes: Vec<WeztermPane>) -> Vec<ShownWeztermPane> {
    let by_window = panes.into_iter().fold(
        BTreeMap::<u64, Vec<WeztermPane>>::new(),
        |mut by_window, pane| {
            by_window.entry(pane.window_id).or_default().push(pane);
            by_window
        },
    );
    by_window
        .into_iter()
        .filter_map(|(wezterm_window, window_panes)| {
            let window_ptys = window_panes
                .iter()
                .filter_map(|pane| pane.tty_name.clone())
                .collect();
            let mut shown = window_panes
                .into_iter()
                .filter(|pane| pane.is_active && pane.title == pane.window_title);
            let shown_pane = shown.next()?;
            if shown.next().is_some() {
                return None;
            }
            Some(ShownWeztermPane {
                wezterm_window,
                pty_path: shown_pane.tty_name?,
                title: shown_pane.title,
                window_ptys,
            })
        })
        .collect()
}

fn socket_of(wezterm_pid: i32) -> Option<PathBuf> {
    child_pids(wezterm_pid).into_iter().find_map(|child| {
        let environment = Proc::new(child)
            .and_then(|child_process| child_process.environ())
            .inspect_err(|environ_failure| {
                tracing::debug!(
                    "wezterm {wezterm_pid}: environment of child {child}: {environ_failure}"
                );
            })
            .ok()?;
        let socket = environment.get(OsStr::new(SOCKET_VARIABLE))?;
        Some(PathBuf::from(OsStr::from_bytes(socket.as_bytes())))
    })
}

fn list_panes(wezterm_pid: i32) -> io::Result<Vec<WeztermPane>> {
    let gui_binary = Proc::new(wezterm_pid)
        .map_err(io::Error::other)?
        .exe()
        .map_err(io::Error::other)?;
    let cli_binary = gui_binary.with_file_name(CLI_BINARY);
    let socket = socket_of(wezterm_pid)
        .ok_or_else(|| io::Error::other("wezterm exposes no unix socket to its shells"))?;
    let listing = Command::new(cli_binary)
        .env(SOCKET_VARIABLE, socket)
        .args(["cli", "--no-auto-start", "list", "--format", "json"])
        .output()?;
    if !listing.status.success() {
        return Err(io::Error::other(
            String::from_utf8(listing.stderr).map_err(io::Error::other)?,
        ));
    }
    Ok(serde_json::from_slice(&listing.stdout)?)
}

pub fn terminal_windows(wezterm_pid: i32) -> io::Result<Vec<TerminalWindow>> {
    let panes = list_panes(wezterm_pid)?;
    let sessions = shell_sessions(wezterm_pid);
    Ok(shown_panes(panes)
        .into_iter()
        .filter_map(|shown_pane| {
            let session = sessions
                .iter()
                .find(|session| session.pty_path == shown_pane.pty_path)?;
            Some(TerminalWindow {
                key: TerminalWindowKey {
                    process: wezterm_pid,
                    terminal_window: shown_pane.wezterm_window,
                },
                probe_target: TitleProbeTarget::WeztermPane {
                    pty_path: shown_pane.pty_path,
                    title: shown_pane.title,
                },
                shown_pid: session.shell_pid,
                pids: sessions
                    .iter()
                    .filter(|window_session| {
                        shown_pane.window_ptys.contains(&window_session.pty_path)
                    })
                    .map(|window_session| window_session.shell_pid)
                    .collect(),
                label_reading: TerminalLabelReading::ShellSession(session.clone()),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ShownWeztermPane, WeztermPane, shown_panes};

    const LISTING: &str = r#"[
      {"window_id": 0, "tab_id": 0, "pane_id": 0, "title": "~", "window_title": "~",
       "cwd": "file://ryzen/home/user", "is_active": true, "tty_name": "/dev/pts/6"},
      {"window_id": 1, "tab_id": 1, "pane_id": 1, "title": "~/proj", "window_title": "~/proj",
       "cwd": "file://ryzen/home/user/proj", "is_active": true, "tty_name": "/dev/pts/7"},
      {"window_id": 1, "tab_id": 2, "pane_id": 2, "title": "htop", "window_title": "~/proj",
       "cwd": "file://ryzen/home/user", "is_active": true, "tty_name": "/dev/pts/8"},
      {"window_id": 2, "tab_id": 3, "pane_id": 3, "title": "~", "window_title": "~",
       "cwd": "file://ryzen/home/user", "is_active": true, "tty_name": "/dev/pts/9"},
      {"window_id": 2, "tab_id": 4, "pane_id": 4, "title": "~", "window_title": "~",
       "cwd": "file://ryzen/home/user", "is_active": true, "tty_name": "/dev/pts/10"}
    ]"#;

    #[test]
    fn each_window_is_described_by_the_one_pane_it_shows() {
        let panes: Vec<WeztermPane> = serde_json::from_str(LISTING).unwrap();
        assert_eq!(
            shown_panes(panes),
            [
                ShownWeztermPane {
                    wezterm_window: 0,
                    pty_path: PathBuf::from("/dev/pts/6"),
                    title: "~".to_owned(),
                    window_ptys: vec![PathBuf::from("/dev/pts/6")],
                },
                ShownWeztermPane {
                    wezterm_window: 1,
                    pty_path: PathBuf::from("/dev/pts/7"),
                    title: "~/proj".to_owned(),
                    window_ptys: vec![PathBuf::from("/dev/pts/7"), PathBuf::from("/dev/pts/8")],
                },
            ]
        );
    }
}
