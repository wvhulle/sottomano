mod kitty_remote_control;
mod pty_shell_sessions;
mod pty_title_escape;
mod shell_prompt_title;
pub mod terminal_shell_label;
#[cfg(test)]
mod terminal_window_tests;
mod title_probe_pairing;
mod wezterm_cli_listing;

use std::{
    collections::HashMap,
    env, io,
    path::{Path, PathBuf},
    time::Instant,
};

use futures::{Stream, StreamExt, future::ready, stream};
use futures_rx::RxExt;
use pty_shell_sessions::ShellSession;
use sottomano_compositor_model::compositor_window_state::{
    CompositorChange, CompositorWindowState,
};
use sottomano_compositors::compositor_connection::compositor_updates;
use sottomano_toml_config::output_windows::{TerminalAgentUse, WindowAgentsConfig};
use terminal_shell_label::TerminalShellLabel;
use title_probe_pairing::Pairing;
use tokio::task::spawn_blocking;

const KITTY_APP_ID: &str = "kitty";

const WEZTERM_APP_ID: &str = "org.wezfurlong.wezterm";

const PTY_SHELL_APP_IDS: [&str; 3] = ["foot", "Alacritty", "com.mitchellh.ghostty"];

#[derive(Debug, Clone, PartialEq, Eq)]
enum PairedTerminalLabel {
    Labelled(TerminalShellLabel),
    Unlabelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PairedWindow {
    shown: i32,
    pids: Vec<i32>,
    terminal_label: PairedTerminalLabel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowProcesses {
    by_window: HashMap<u64, PairedWindow>,
}

impl WindowProcesses {
    #[must_use]
    pub fn with_terminal_label(
        mut self,
        window: u64,
        shown_pid: i32,
        terminal_label: TerminalShellLabel,
    ) -> Self {
        self.by_window.insert(
            window,
            PairedWindow {
                shown: shown_pid,
                pids: vec![shown_pid],
                terminal_label: PairedTerminalLabel::Labelled(terminal_label),
            },
        );
        self
    }

    #[must_use]
    pub fn window_terminal_pids(&self, window: u64) -> Option<&[i32]> {
        self.by_window
            .get(&window)
            .map(|paired| paired.pids.as_slice())
    }

    #[must_use]
    pub fn shown_in(&self, window: u64) -> Option<i32> {
        self.by_window.get(&window).map(|paired| paired.shown)
    }

    #[must_use]
    pub fn terminal_label_in(&self, window: u64) -> Option<&TerminalShellLabel> {
        match &self.by_window.get(&window)?.terminal_label {
            PairedTerminalLabel::Labelled(terminal_label) => Some(terminal_label),
            PairedTerminalLabel::Unlabelled => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TerminalWindowKey {
    process: i32,
    terminal_window: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TitleProbeTarget {
    KittyWindow {
        socket: PathBuf,
        kitty_window: u64,
        title: String,
    },
    WeztermPane {
        pty_path: PathBuf,
        title: String,
    },
    ShellPty {
        pty_path: PathBuf,
    },
}

impl TitleProbeTarget {
    fn known_title(&self) -> Option<&str> {
        match self {
            TitleProbeTarget::KittyWindow { title, .. }
            | TitleProbeTarget::WeztermPane { title, .. } => Some(title),
            TitleProbeTarget::ShellPty { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TerminalLabelSource {
    Unlabelled,
    Labelled(TerminalShellLabel),
    ShellSession(ShellSession),
}

impl TerminalLabelSource {
    fn terminal_label(&self, compositor_title: &str, home: &Path) -> PairedTerminalLabel {
        match self {
            TerminalLabelSource::Unlabelled => PairedTerminalLabel::Unlabelled,
            TerminalLabelSource::Labelled(label) => PairedTerminalLabel::Labelled(label.clone()),
            TerminalLabelSource::ShellSession(session) => {
                session.terminal_label(compositor_title, home).map_or(
                    PairedTerminalLabel::Unlabelled,
                    PairedTerminalLabel::Labelled,
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalWindow {
    key: TerminalWindowKey,
    probe_target: TitleProbeTarget,
    shown_pid: i32,
    pids: Vec<i32>,
    label_source: TerminalLabelSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalAgent {
    KittyRemoteControl,
    WeztermCli,
    PtyShells,
}

impl TerminalAgent {
    fn serving(app_id: &str, agents: &WindowAgentsConfig) -> Option<Self> {
        match app_id {
            KITTY_APP_ID if agents.kitty == TerminalAgentUse::Enabled => {
                Some(TerminalAgent::KittyRemoteControl)
            }
            WEZTERM_APP_ID if agents.wezterm == TerminalAgentUse::Enabled => {
                Some(TerminalAgent::WeztermCli)
            }
            _ if agents.pty_shells == TerminalAgentUse::Enabled
                && PTY_SHELL_APP_IDS.contains(&app_id) =>
            {
                Some(TerminalAgent::PtyShells)
            }
            _ => None,
        }
    }
}

trait TerminalAgentAccess {
    fn list_terminal_windows(
        &mut self,
        agent: TerminalAgent,
        process: i32,
        compositor_window_count: usize,
    ) -> io::Result<Vec<TerminalWindow>>;

    fn hold_title(&mut self, target: &TitleProbeTarget, marker: &str) -> io::Result<()>;

    fn yield_title(&mut self, target: &TitleProbeTarget, title: &str) -> io::Result<()>;
}

struct LiveTerminalAgents {
    home: PathBuf,
}

impl TerminalAgentAccess for LiveTerminalAgents {
    fn list_terminal_windows(
        &mut self,
        agent: TerminalAgent,
        process: i32,
        compositor_window_count: usize,
    ) -> io::Result<Vec<TerminalWindow>> {
        match agent {
            TerminalAgent::KittyRemoteControl => {
                let socket = kitty_remote_control::kitty_socket_path(process)
                    .ok_or_else(|| io::Error::other("kitty exposes no remote control socket"))?;
                kitty_remote_control::list(&socket, process, &self.home)
            }
            TerminalAgent::WeztermCli => wezterm_cli_listing::terminal_windows(process),
            TerminalAgent::PtyShells => Ok(pty_shell_sessions::terminal_windows(
                process,
                compositor_window_count,
            )),
        }
    }

    fn hold_title(&mut self, target: &TitleProbeTarget, marker: &str) -> io::Result<()> {
        match target {
            TitleProbeTarget::KittyWindow {
                socket,
                kitty_window,
                ..
            } => kitty_remote_control::hold_title(socket, *kitty_window, marker),
            TitleProbeTarget::WeztermPane { pty_path, .. }
            | TitleProbeTarget::ShellPty { pty_path } => {
                pty_title_escape::write_title(pty_path, marker)
            }
        }
    }

    fn yield_title(&mut self, target: &TitleProbeTarget, title: &str) -> io::Result<()> {
        match target {
            TitleProbeTarget::KittyWindow {
                socket,
                kitty_window,
                ..
            } => kitty_remote_control::yield_title(socket, *kitty_window, title),
            TitleProbeTarget::WeztermPane { pty_path, .. }
            | TitleProbeTarget::ShellPty { pty_path } => {
                pty_title_escape::write_title(pty_path, title)
            }
        }
    }
}

struct PairingWorker {
    pairing: Pairing,
    access: LiveTerminalAgents,
}

impl PairingWorker {
    async fn refreshed(
        mut self,
        compositor: CompositorWindowState,
    ) -> Option<(WindowProcesses, Self)> {
        spawn_blocking(move || {
            let processes = self
                .pairing
                .refresh(&compositor, &mut self.access, Instant::now());
            (processes, self)
        })
        .await
        .inspect_err(|join_failure| {
            tracing::warn!("window pairing: refresh task failed: {join_failure}");
        })
        .ok()
    }
}

pub fn window_pairings(agents: WindowAgentsConfig) -> impl Stream<Item = WindowProcesses> + Send {
    let home = env::home_dir();
    if home.is_none() {
        tracing::warn!("window pairing stopped: no home directory");
    }
    stream::iter(home.map(|home| paired_window_processes(home, agents)))
        .flatten()
        .distinct_until_changed()
}

fn paired_window_processes(
    home: PathBuf,
    agents: WindowAgentsConfig,
) -> impl Stream<Item = WindowProcesses> + Send {
    let worker = PairingWorker {
        pairing: Pairing::new(home.clone(), agents),
        access: LiveTerminalAgents { home },
    };
    let window_updates = compositor_updates()
        .filter(|update| ready(update.changes.contains(&CompositorChange::Windows)))
        .boxed();
    stream::unfold(
        (window_updates, worker),
        |(mut window_updates, worker)| async move {
            let update = window_updates.next().await?;
            let (processes, worker) = worker.refreshed(update.state).await?;
            Some((processes, (window_updates, worker)))
        },
    )
}
