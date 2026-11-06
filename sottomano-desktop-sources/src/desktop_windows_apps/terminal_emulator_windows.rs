mod kitty_remote_control;
mod pty_shell_sessions;
mod pty_title_escape;
mod shell_prompt_title;
pub mod terminal_shell_label;
mod wezterm_cli_listing;

use std::{
    collections::{HashMap, HashSet},
    env, io, mem,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use futures::Stream;
use niri_ipc::{
    Event, Request, Response,
    socket::Socket,
    state::{EventStreamState, EventStreamStatePart},
};
use pty_shell_sessions::ShellSession;
use sottomano_toml_config::widget_groups::WindowAgentsConfig;
use terminal_shell_label::TerminalShellLabel;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::stream_operators::StreamOperators;

const KITTY_APP_ID: &str = "kitty";

const WEZTERM_APP_ID: &str = "org.wezfurlong.wezterm";

const PTY_SHELL_APP_IDS: [&str; 3] = ["foot", "Alacritty", "com.mitchellh.ghostty"];

const PROBE_PATIENCE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
struct PairedWindow {
    shown: i32,
    pids: Vec<i32>,
    terminal_label: Option<TerminalShellLabel>,
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
                terminal_label: Some(terminal_label),
            },
        );
        self
    }

    #[must_use]
    pub fn of_window(&self, window: u64) -> Option<&[i32]> {
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
        self.by_window.get(&window)?.terminal_label.as_ref()
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
enum TerminalLabelReading {
    Unlabelled,
    Labelled(TerminalShellLabel),
    ShellSession(ShellSession),
}

impl TerminalLabelReading {
    fn terminal_label(&self, niri_title: &str, home: &Path) -> Option<TerminalShellLabel> {
        match self {
            TerminalLabelReading::Unlabelled => None,
            TerminalLabelReading::Labelled(label) => Some(label.clone()),
            TerminalLabelReading::ShellSession(session) => session.terminal_label(niri_title, home),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalWindow {
    key: TerminalWindowKey,
    probe_target: TitleProbeTarget,
    shown_pid: i32,
    pids: Vec<i32>,
    label_reading: TerminalLabelReading,
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
            KITTY_APP_ID if agents.kitty => Some(TerminalAgent::KittyRemoteControl),
            WEZTERM_APP_ID if agents.wezterm => Some(TerminalAgent::WeztermCli),
            _ if agents.pty_shells && PTY_SHELL_APP_IDS.contains(&app_id) => {
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
        niri_window_count: usize,
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
        niri_window_count: usize,
    ) -> io::Result<Vec<TerminalWindow>> {
        match agent {
            TerminalAgent::KittyRemoteControl => {
                let socket = kitty_remote_control::socket_of(process)
                    .ok_or_else(|| io::Error::other("kitty exposes no remote control socket"))?;
                kitty_remote_control::list(&socket, process, &self.home)
            }
            TerminalAgent::WeztermCli => wezterm_cli_listing::terminal_windows(process),
            TerminalAgent::PtyShells => Ok(pty_shell_sessions::terminal_windows(
                process,
                niri_window_count,
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

struct TerminalProcess {
    agent: TerminalAgent,
    niri_windows: Vec<u64>,
}

struct PendingProbe {
    key: TerminalWindowKey,
    marker: String,
    target: TitleProbeTarget,
    titles_before: HashMap<u64, String>,
    sent_at: Instant,
}

struct Pairing {
    paired: HashMap<u64, TerminalWindowKey>,
    pending: Vec<PendingProbe>,
    unpairable: HashSet<TerminalWindowKey>,
    probes_sent: u64,
    home: PathBuf,
    agents: WindowAgentsConfig,
}

impl Pairing {
    fn new(home: PathBuf, agents: WindowAgentsConfig) -> Self {
        Self {
            paired: HashMap::new(),
            pending: Vec::new(),
            unpairable: HashSet::new(),
            probes_sent: 0,
            home,
            agents,
        }
    }

    fn refresh(
        &mut self,
        niri: &EventStreamState,
        access: &mut impl TerminalAgentAccess,
        now: Instant,
    ) -> WindowProcesses {
        let open_windows: HashSet<u64> = niri.windows.windows.keys().copied().collect();
        self.paired
            .retain(|niri_window, _| open_windows.contains(niri_window));
        let terminal_processes = self.terminal_processes(niri);
        self.pending
            .retain(|probe| terminal_processes.contains_key(&probe.key.process));
        self.unpairable
            .retain(|key| terminal_processes.contains_key(&key.process));
        let mut processes = WindowProcesses::default();
        for (&process, terminal) in &terminal_processes {
            let paired_windows = self.pair_process(process, terminal, niri, access, now);
            processes.by_window.extend(paired_windows);
        }
        processes
    }

    fn pair_process(
        &mut self,
        process: i32,
        terminal: &TerminalProcess,
        niri: &EventStreamState,
        access: &mut impl TerminalAgentAccess,
        now: Instant,
    ) -> HashMap<u64, PairedWindow> {
        let terminal_windows = match access.list_terminal_windows(
            terminal.agent,
            process,
            terminal.niri_windows.len(),
        ) {
            Ok(terminal_windows) => terminal_windows,
            Err(listing_error) => {
                tracing::debug!("terminal {process} unreachable: {listing_error}");
                return HashMap::new();
            }
        };
        self.forget_closed(process, &terminal_windows);
        self.settle_probes(process, niri, access, now);
        if let ([sole_niri_window], [sole_terminal_window]) = (
            terminal.niri_windows.as_slice(),
            terminal_windows.as_slice(),
        ) {
            self.paired
                .insert(*sole_niri_window, sole_terminal_window.key);
        }
        for terminal_window in &terminal_windows {
            if self.awaits_probe(terminal_window.key) {
                self.start_probe(terminal_window, niri, access, now);
            }
        }
        terminal_windows
            .iter()
            .flat_map(|terminal_window| {
                self.niri_windows_paired_with(terminal_window.key)
                    .into_iter()
                    .map(|niri_window| {
                        let niri_title = window_title(niri, niri_window).unwrap_or_default();
                        let paired_window = PairedWindow {
                            shown: terminal_window.shown_pid,
                            pids: terminal_window.pids.clone(),
                            terminal_label: terminal_window
                                .label_reading
                                .terminal_label(niri_title, &self.home),
                        };
                        (niri_window, paired_window)
                    })
            })
            .collect()
    }

    fn terminal_processes(&self, niri: &EventStreamState) -> HashMap<i32, TerminalProcess> {
        niri.windows
            .windows
            .values()
            .filter_map(|window| {
                let agent = TerminalAgent::serving(window.app_id.as_deref()?, &self.agents)?;
                Some((window.pid?, agent, window.id))
            })
            .fold(
                HashMap::new(),
                |mut by_process, (process, agent, niri_window)| {
                    by_process
                        .entry(process)
                        .or_insert_with(|| TerminalProcess {
                            agent,
                            niri_windows: Vec::new(),
                        })
                        .niri_windows
                        .push(niri_window);
                    by_process
                },
            )
    }

    fn forget_closed(&mut self, process: i32, terminal_windows: &[TerminalWindow]) {
        let is_listed = |key: &TerminalWindowKey| {
            key.process != process || terminal_windows.iter().any(|listed| listed.key == *key)
        };
        self.paired.retain(|_, key| is_listed(key));
        self.unpairable.retain(is_listed);
        self.pending.retain(|probe| is_listed(&probe.key));
    }

    fn awaits_probe(&self, key: TerminalWindowKey) -> bool {
        !self.paired.values().any(|paired| *paired == key)
            && !self.pending.iter().any(|probe| probe.key == key)
            && !self.unpairable.contains(&key)
    }

    fn niri_windows_paired_with(&self, key: TerminalWindowKey) -> Vec<u64> {
        self.paired
            .iter()
            .filter(|(_, paired)| **paired == key)
            .map(|(&niri_window, _)| niri_window)
            .collect()
    }

    fn start_probe(
        &mut self,
        terminal_window: &TerminalWindow,
        niri: &EventStreamState,
        access: &mut impl TerminalAgentAccess,
        now: Instant,
    ) {
        self.probes_sent += 1;
        let key = terminal_window.key;
        let marker = format!("sottomano-probe-{}-{}", key.process, self.probes_sent);
        if let Err(hold_error) = access.hold_title(&terminal_window.probe_target, &marker) {
            tracing::debug!(
                "terminal {} refused a probe title: {hold_error}",
                key.process
            );
            self.unpairable.insert(key);
            return;
        }
        self.pending.push(PendingProbe {
            key,
            marker,
            target: terminal_window.probe_target.clone(),
            titles_before: windows_of_process(niri, key.process)
                .filter_map(|window| Some((window.id, window.title.clone()?)))
                .collect(),
            sent_at: now,
        });
    }

    fn settle_probes(
        &mut self,
        process: i32,
        niri: &EventStreamState,
        access: &mut impl TerminalAgentAccess,
        now: Instant,
    ) {
        for probe in mem::take(&mut self.pending) {
            if probe.key.process != process {
                self.pending.push(probe);
                continue;
            }
            let marked_window = windows_of_process(niri, process)
                .find(|window| window.title.as_deref() == Some(probe.marker.as_str()))
                .map(|window| window.id);
            match marked_window {
                Some(niri_window) => {
                    let restored_title = probe
                        .target
                        .known_title()
                        .or_else(|| probe.titles_before.get(&niri_window).map(String::as_str));
                    if let Some(restored_title) = restored_title {
                        restore_title(access, &probe.target, restored_title);
                    }
                    self.paired.insert(niri_window, probe.key);
                }
                None if now.duration_since(probe.sent_at) >= PROBE_PATIENCE => {
                    if let Some(known_title) = probe.target.known_title() {
                        restore_title(access, &probe.target, known_title);
                    }
                    self.unpairable.insert(probe.key);
                }
                None => self.pending.push(probe),
            }
        }
    }
}

fn restore_title(access: &mut impl TerminalAgentAccess, target: &TitleProbeTarget, title: &str) {
    if let Err(yield_error) = access.yield_title(target, title) {
        tracing::warn!("terminal kept the probe title: {yield_error}");
    }
}

fn windows_of_process(
    niri: &EventStreamState,
    process: i32,
) -> impl Iterator<Item = &niri_ipc::Window> {
    niri.windows
        .windows
        .values()
        .filter(move |window| window.pid == Some(process))
}

fn window_title(niri: &EventStreamState, niri_window: u64) -> Option<&str> {
    niri.windows.windows.get(&niri_window)?.title.as_deref()
}

fn changes_windows(event: &Event) -> bool {
    matches!(
        event,
        Event::WindowsChanged { .. }
            | Event::WindowOpenedOrChanged { .. }
            | Event::WindowClosed { .. }
    )
}

fn pair_windows(
    sender: &mpsc::Sender<WindowProcesses>,
    agents: WindowAgentsConfig,
) -> io::Result<()> {
    let mut socket = Socket::connect()?;
    match socket.send(Request::EventStream)? {
        Ok(Response::Handled) => {}
        Ok(other) => return Err(io::Error::other(format!("unexpected reply {other:?}"))),
        Err(rejection) => return Err(io::Error::other(rejection)),
    }
    let mut read_event = socket.read_events();
    let home = env::home_dir().ok_or_else(|| io::Error::other("no home directory"))?;
    let mut access = LiveTerminalAgents { home: home.clone() };
    let mut pairing = Pairing::new(home, agents);
    let mut niri = EventStreamState::default();
    loop {
        let event = read_event()?;
        let windows_changed = changes_windows(&event);
        niri.apply(event);
        if windows_changed {
            let processes = pairing.refresh(&niri, &mut access, Instant::now());
            if sender.blocking_send(processes).is_err() {
                return Ok(());
            }
        }
    }
}

pub fn window_pairings(agents: WindowAgentsConfig) -> impl Stream<Item = WindowProcesses> + Send {
    let (sender, receiver) = mpsc::channel(16);
    thread::spawn(move || {
        if let Err(pairing_error) = pair_windows(&sender, agents) {
            tracing::warn!("window pairing stopped: {pairing_error}");
        }
    });
    ReceiverStream::new(receiver).distinct_until_changed()
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };

    use niri_ipc::{Window, state::EventStreamState};
    use sottomano_toml_config::widget_groups::WindowAgentsConfig;

    use super::{
        Pairing, TerminalAgent, TerminalAgentAccess, TerminalLabelReading, TerminalWindow,
        TerminalWindowKey, TitleProbeTarget,
    };
    use crate::desktop_windows_apps::niri_test_windows::unplaced_niri_window;

    const GHOSTTY_PID: i32 = 700;

    #[derive(Default)]
    struct ScriptedTerminal {
        listed: Vec<TerminalWindow>,
        held_titles: Vec<(PathBuf, String)>,
        yielded_titles: Vec<(PathBuf, String)>,
    }

    fn pty_of(target: &TitleProbeTarget) -> PathBuf {
        match target {
            TitleProbeTarget::KittyWindow { socket, .. } => socket.clone(),
            TitleProbeTarget::WeztermPane { pty_path, .. }
            | TitleProbeTarget::ShellPty { pty_path } => pty_path.clone(),
        }
    }

    impl TerminalAgentAccess for ScriptedTerminal {
        fn list_terminal_windows(
            &mut self,
            _agent: TerminalAgent,
            _process: i32,
            _niri_window_count: usize,
        ) -> io::Result<Vec<TerminalWindow>> {
            Ok(self.listed.clone())
        }

        fn hold_title(&mut self, target: &TitleProbeTarget, marker: &str) -> io::Result<()> {
            self.held_titles.push((pty_of(target), marker.to_owned()));
            Ok(())
        }

        fn yield_title(&mut self, target: &TitleProbeTarget, title: &str) -> io::Result<()> {
            self.yielded_titles.push((pty_of(target), title.to_owned()));
            Ok(())
        }
    }

    fn shell_on_pty(pty_number: u64) -> TerminalWindow {
        TerminalWindow {
            key: TerminalWindowKey {
                process: GHOSTTY_PID,
                terminal_window: pty_number,
            },
            probe_target: TitleProbeTarget::ShellPty {
                pty_path: PathBuf::from(format!("/dev/pts/{pty_number}")),
            },
            shown_pid: i32::try_from(pty_number).unwrap() + 1000,
            pids: vec![i32::try_from(pty_number).unwrap() + 1000],
            label_reading: TerminalLabelReading::Unlabelled,
        }
    }

    fn ghostty_windows(titles: &[(u64, &str)]) -> EventStreamState {
        let mut niri = EventStreamState::default();
        for &(niri_window, title) in titles {
            niri.windows.windows.insert(
                niri_window,
                Window {
                    pid: Some(GHOSTTY_PID),
                    app_id: Some("com.mitchellh.ghostty".to_owned()),
                    title: Some(title.to_owned()),
                    ..unplaced_niri_window(niri_window)
                },
            );
        }
        niri
    }

    fn pairing() -> Pairing {
        Pairing::new(PathBuf::from("/home/user"), WindowAgentsConfig::default())
    }

    fn held_marker(terminal: &ScriptedTerminal, pty: &str) -> String {
        terminal
            .held_titles
            .iter()
            .find(|(held_pty, _)| held_pty == Path::new(pty))
            .map(|(_, marker)| marker.clone())
            .unwrap()
    }

    #[test]
    fn a_sole_window_is_paired_without_writing_a_title() {
        let mut terminal = ScriptedTerminal {
            listed: vec![shell_on_pty(3)],
            ..ScriptedTerminal::default()
        };
        let processes =
            pairing().refresh(&ghostty_windows(&[(1, "~")]), &mut terminal, Instant::now());
        assert_eq!(processes.shown_in(1), Some(1003));
        assert!(terminal.held_titles.is_empty());
    }

    #[test]
    fn shared_process_windows_are_paired_by_the_window_that_shows_the_probe() {
        let mut terminal = ScriptedTerminal {
            listed: vec![shell_on_pty(3), shell_on_pty(4)],
            ..ScriptedTerminal::default()
        };
        let mut pairing = pairing();
        let started = Instant::now();
        let before = ghostty_windows(&[(1, "~/a"), (2, "~/b")]);
        let unpaired = pairing.refresh(&before, &mut terminal, started);
        assert_eq!(unpaired.shown_in(1), None);
        assert_eq!(terminal.held_titles.len(), 2);
        let probe_marker = held_marker(&terminal, "/dev/pts/4");
        let showing_probe = ghostty_windows(&[(1, "~/a"), (2, &probe_marker)]);
        let processes = pairing.refresh(&showing_probe, &mut terminal, started);
        assert_eq!(processes.shown_in(2), Some(1004));
        assert_eq!(processes.shown_in(1), None);
        assert_eq!(
            terminal.yielded_titles,
            [(PathBuf::from("/dev/pts/4"), "~/b".to_owned())]
        );
        assert_eq!(terminal.held_titles.len(), 2);
    }

    #[test]
    fn a_probe_nobody_shows_gives_up_and_is_not_repeated() {
        let mut terminal = ScriptedTerminal {
            listed: vec![shell_on_pty(3), shell_on_pty(4)],
            ..ScriptedTerminal::default()
        };
        let mut pairing = pairing();
        let started = Instant::now();
        let niri = ghostty_windows(&[(1, "~/a"), (2, "~/b")]);
        pairing.refresh(&niri, &mut terminal, started);
        let later = started + Duration::from_secs(3);
        let processes = pairing.refresh(&niri, &mut terminal, later);
        pairing.refresh(&niri, &mut terminal, later);
        assert_eq!(processes.shown_in(1), None);
        assert_eq!(terminal.held_titles.len(), 2);
        assert!(terminal.yielded_titles.is_empty());
    }

    #[test]
    fn disabled_agents_leave_their_terminals_alone() {
        let mut terminal = ScriptedTerminal {
            listed: vec![shell_on_pty(3)],
            ..ScriptedTerminal::default()
        };
        let mut pairing = Pairing::new(
            PathBuf::from("/home/user"),
            WindowAgentsConfig {
                pty_shells: false,
                ..WindowAgentsConfig::default()
            },
        );
        let processes =
            pairing.refresh(&ghostty_windows(&[(1, "~")]), &mut terminal, Instant::now());
        assert_eq!(processes, super::WindowProcesses::default());
        assert!(pairing.paired.is_empty());
    }
}
