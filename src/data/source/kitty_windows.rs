mod kitty_remote;
pub mod kitty_window_label;
mod shell_title;

use std::{
    collections::{HashMap, HashSet},
    env, io,
    path::{Path, PathBuf},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use iced::Subscription;
use kitty_window_label::KittyWindowLabel;
use niri_ipc::{
    Event, Request, Response, Window,
    socket::Socket,
    state::{EventStreamState, EventStreamStatePart},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{config::widget_groups::WindowAgentsConfig, stream_operators::StreamOperators};

pub const KITTY_APP_ID: &str = "kitty";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentWindow {
    pub id: u64,
    pub title_holder: u64,
    pub title: String,
    pub shown_pid: i32,
    pub pids: Vec<i32>,
    pub kitty_label: Option<KittyWindowLabel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PairedWindow {
    shown: i32,
    pids: Vec<i32>,
    kitty_label: Option<KittyWindowLabel>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowProcesses {
    by_window: HashMap<u64, PairedWindow>,
}

impl WindowProcesses {
    pub fn of_window(&self, window: u64) -> Option<&[i32]> {
        self.by_window
            .get(&window)
            .map(|paired| paired.pids.as_slice())
    }

    pub fn shown_in(&self, window: u64) -> Option<i32> {
        self.by_window.get(&window).map(|paired| paired.shown)
    }

    pub fn kitty_label_in(&self, window: u64) -> Option<&KittyWindowLabel> {
        self.by_window.get(&window)?.kitty_label.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AgentWindowKey {
    process: i32,
    window: u64,
}

struct NiriWindows<R> {
    state: EventStreamState,
    read: R,
}

impl<R> NiriWindows<R> {
    fn of_process(&self, process: i32) -> impl Iterator<Item = &Window> {
        self.state
            .windows
            .windows
            .values()
            .filter(move |window| window.pid == Some(process))
    }
}

struct Pairing {
    paired: HashMap<u64, AgentWindowKey>,
    home: PathBuf,
}

impl Pairing {
    fn refresh<R: FnMut() -> io::Result<Event>>(
        &mut self,
        niri: &mut NiriWindows<R>,
    ) -> io::Result<WindowProcesses> {
        let open: HashSet<u64> = niri.state.windows.windows.keys().copied().collect();
        self.paired
            .retain(|niri_window, _| open.contains(niri_window));
        let mut processes = WindowProcesses::default();
        for process in kitty_processes(niri.state.windows.windows.values()) {
            let Some(socket) = kitty_remote::socket_of(process) else {
                continue;
            };
            let agent_windows = match kitty_remote::list(&socket, &self.home) {
                Ok(agent_windows) => agent_windows,
                Err(e) => {
                    tracing::debug!("kitty {process} unreachable: {e}");
                    continue;
                }
            };
            self.forget_closed(process, &agent_windows);
            for agent_window in &agent_windows {
                let key = AgentWindowKey {
                    process,
                    window: agent_window.id,
                };
                if !self.paired.values().any(|paired| *paired == key) {
                    self.probe(&socket, key, agent_window, niri)?;
                }
            }
            for agent_window in &agent_windows {
                let key = AgentWindowKey {
                    process,
                    window: agent_window.id,
                };
                let paired_niri_windows = self
                    .paired
                    .iter()
                    .filter(|(_, paired)| **paired == key)
                    .map(|(&niri_window, _)| niri_window);
                for niri_window in paired_niri_windows {
                    processes.by_window.insert(
                        niri_window,
                        PairedWindow {
                            shown: agent_window.shown_pid,
                            pids: agent_window.pids.clone(),
                            kitty_label: agent_window.kitty_label.clone(),
                        },
                    );
                }
            }
        }
        Ok(processes)
    }

    fn forget_closed(&mut self, process: i32, agent_windows: &[AgentWindow]) {
        self.paired.retain(|_, key| {
            key.process != process || agent_windows.iter().any(|window| window.id == key.window)
        });
    }

    fn probe<R: FnMut() -> io::Result<Event>>(
        &mut self,
        socket: &Path,
        key: AgentWindowKey,
        agent_window: &AgentWindow,
        niri: &mut NiriWindows<R>,
    ) -> io::Result<()> {
        let marker = probe_marker();
        if let Err(e) = kitty_remote::hold_title(socket, agent_window, &marker) {
            tracing::debug!("kitty {} refused a title: {e}", key.process);
            return Ok(());
        }
        let titled = self.await_marker(key.process, &marker, niri);
        if let Err(e) = kitty_remote::yield_title(socket, agent_window, &agent_window.title) {
            tracing::warn!("kitty {} kept the probe title: {e}", key.process);
        }
        if let Some(niri_window) = titled? {
            self.paired.insert(niri_window, key);
        }
        Ok(())
    }

    fn await_marker<R: FnMut() -> io::Result<Event>>(
        &self,
        process: i32,
        marker: &str,
        niri: &mut NiriWindows<R>,
    ) -> io::Result<Option<u64>> {
        loop {
            if let Some(titled) = niri
                .of_process(process)
                .find(|window| window.title.as_deref() == Some(marker))
            {
                return Ok(Some(titled.id));
            }
            let event = (niri.read)()?;
            if let Event::WindowClosed { id } = &event
                && niri.of_process(process).any(|window| window.id == *id)
                && !self.paired.contains_key(id)
            {
                niri.state.apply(event);
                return Ok(None);
            }
            niri.state.apply(event);
        }
    }
}

fn kitty_processes<'a>(windows: impl Iterator<Item = &'a Window>) -> HashSet<i32> {
    windows
        .filter(|window| window.app_id.as_deref() == Some(KITTY_APP_ID))
        .filter_map(|window| window.pid)
        .collect()
}

fn probe_marker() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("vetrata-probe-{nanos}")
}

fn changes_windows(event: &Event) -> bool {
    matches!(
        event,
        Event::WindowsChanged { .. }
            | Event::WindowOpenedOrChanged { .. }
            | Event::WindowClosed { .. }
    )
}

fn pair_windows(sender: &mpsc::Sender<WindowProcesses>) -> io::Result<()> {
    let mut socket = Socket::connect()?;
    match socket.send(Request::EventStream)? {
        Ok(Response::Handled) => {}
        Ok(other) => return Err(io::Error::other(format!("unexpected reply {other:?}"))),
        Err(rejection) => return Err(io::Error::other(rejection)),
    }
    let mut niri = NiriWindows {
        state: EventStreamState::default(),
        read: socket.read_events(),
    };
    let mut pairing = Pairing {
        paired: HashMap::new(),
        home: env::home_dir().ok_or_else(|| io::Error::other("no home directory"))?,
    };
    loop {
        let event = (niri.read)()?;
        let windows_changed = changes_windows(&event);
        niri.state.apply(event);
        if windows_changed {
            let processes = pairing.refresh(&mut niri)?;
            if sender.blocking_send(processes).is_err() {
                return Ok(());
            }
        }
    }
}

pub fn subscription(config: &WindowAgentsConfig) -> Subscription<WindowProcesses> {
    if !config.kitty {
        return Subscription::none();
    }
    Subscription::run_with(config.clone(), |_| {
        let (sender, receiver) = mpsc::channel(16);
        thread::spawn(move || {
            if let Err(e) = pair_windows(&sender) {
                tracing::warn!("window pairing stopped: {e}");
            }
        });
        ReceiverStream::new(receiver).distinct_until_changed()
    })
}
