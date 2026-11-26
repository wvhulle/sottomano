use std::{
    collections::{HashMap, HashSet},
    mem,
    path::PathBuf,
    time::{Duration, Instant},
};

use sottomano_compositor_model::{
    compositor_window_facts::{WindowAppId, WindowPid, WindowTitle},
    compositor_window_state::{CompositorWindow, CompositorWindowState},
};
use sottomano_toml_config::output_windows::WindowAgentsConfig;

use super::{
    PairedWindow, TerminalAgent, TerminalAgentAccess, TerminalWindow, TerminalWindowKey,
    TitleProbeTarget, WindowProcesses,
};

const PROBE_PATIENCE: Duration = Duration::from_secs(2);

struct TerminalProcess {
    agent: TerminalAgent,
    compositor_windows: Vec<u64>,
}

struct PendingProbe {
    key: TerminalWindowKey,
    marker: String,
    target: TitleProbeTarget,
    titles_before: HashMap<u64, String>,
    sent_at: Instant,
}

pub(super) struct Pairing {
    pub(super) paired: HashMap<u64, TerminalWindowKey>,
    pending: Vec<PendingProbe>,
    unpairable: HashSet<TerminalWindowKey>,
    probes_sent: u64,
    home: PathBuf,
    agents: WindowAgentsConfig,
}

impl Pairing {
    pub(super) fn new(home: PathBuf, agents: WindowAgentsConfig) -> Self {
        Self {
            paired: HashMap::new(),
            pending: Vec::new(),
            unpairable: HashSet::new(),
            probes_sent: 0,
            home,
            agents,
        }
    }

    pub(super) fn refresh(
        &mut self,
        compositor: &CompositorWindowState,
        access: &mut impl TerminalAgentAccess,
        now: Instant,
    ) -> WindowProcesses {
        let open_windows: HashSet<u64> = compositor.windows.keys().copied().collect();
        self.paired
            .retain(|compositor_window, _| open_windows.contains(compositor_window));
        let terminal_processes = self.terminal_processes(compositor);
        self.pending
            .retain(|probe| terminal_processes.contains_key(&probe.key.process));
        self.unpairable
            .retain(|key| terminal_processes.contains_key(&key.process));
        let mut processes = WindowProcesses::default();
        for (&process, terminal) in &terminal_processes {
            let paired_windows = self.pair_process(process, terminal, compositor, access, now);
            processes.by_window.extend(paired_windows);
        }
        processes
    }

    fn pair_process(
        &mut self,
        process: i32,
        terminal: &TerminalProcess,
        compositor: &CompositorWindowState,
        access: &mut impl TerminalAgentAccess,
        now: Instant,
    ) -> HashMap<u64, PairedWindow> {
        let terminal_windows = match access.list_terminal_windows(
            terminal.agent,
            process,
            terminal.compositor_windows.len(),
        ) {
            Ok(terminal_windows) => terminal_windows,
            Err(listing_error) => {
                tracing::debug!("terminal {process} unreachable: {listing_error}");
                return HashMap::new();
            }
        };
        self.forget_closed(process, &terminal_windows);
        self.settle_probes(process, compositor, access, now);
        if let ([sole_compositor_window], [sole_terminal_window]) = (
            terminal.compositor_windows.as_slice(),
            terminal_windows.as_slice(),
        ) {
            self.paired
                .insert(*sole_compositor_window, sole_terminal_window.key);
        }
        for terminal_window in &terminal_windows {
            let key = terminal_window.key;
            let awaits_probe = !self.paired.values().any(|paired| *paired == key)
                && !self.pending.iter().any(|probe| probe.key == key)
                && !self.unpairable.contains(&key);
            if awaits_probe {
                self.start_probe(terminal_window, compositor, access, now);
            }
        }
        terminal_windows
            .iter()
            .flat_map(|terminal_window| {
                self.compositor_windows_paired_with(terminal_window.key)
                    .into_iter()
                    .map(|compositor_window| {
                        let compositor_title =
                            window_title(compositor, compositor_window).unwrap_or_default();
                        let paired_window = PairedWindow {
                            shown: terminal_window.shown_pid,
                            pids: terminal_window.pids.clone(),
                            terminal_label: terminal_window
                                .label_source
                                .terminal_label(compositor_title, &self.home),
                        };
                        (compositor_window, paired_window)
                    })
            })
            .collect()
    }

    fn terminal_processes(
        &self,
        compositor: &CompositorWindowState,
    ) -> HashMap<i32, TerminalProcess> {
        compositor
            .windows
            .values()
            .filter_map(|window| {
                let WindowAppId::Reported(app_id) = &window.app_id else {
                    return None;
                };
                let WindowPid::Reported(pid) = window.pid else {
                    return None;
                };
                let agent = TerminalAgent::serving(app_id, &self.agents)?;
                Some((pid, agent, window.id))
            })
            .fold(
                HashMap::new(),
                |mut by_process, (process, agent, compositor_window)| {
                    by_process
                        .entry(process)
                        .or_insert_with(|| TerminalProcess {
                            agent,
                            compositor_windows: Vec::new(),
                        })
                        .compositor_windows
                        .push(compositor_window);
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

    fn compositor_windows_paired_with(&self, key: TerminalWindowKey) -> Vec<u64> {
        self.paired
            .iter()
            .filter(|(_, paired)| **paired == key)
            .map(|(&compositor_window, _)| compositor_window)
            .collect()
    }

    fn start_probe(
        &mut self,
        terminal_window: &TerminalWindow,
        compositor: &CompositorWindowState,
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
            titles_before: windows_of_process(compositor, key.process)
                .filter_map(|window| match &window.title {
                    WindowTitle::Titled(title) => Some((window.id, title.clone())),
                    WindowTitle::Untitled => None,
                })
                .collect(),
            sent_at: now,
        });
    }

    fn settle_probes(
        &mut self,
        process: i32,
        compositor: &CompositorWindowState,
        access: &mut impl TerminalAgentAccess,
        now: Instant,
    ) {
        for probe in mem::take(&mut self.pending) {
            if probe.key.process != process {
                self.pending.push(probe);
                continue;
            }
            let marked_window = windows_of_process(compositor, process)
                .find(|window| matches!(&window.title, WindowTitle::Titled(title) if *title == probe.marker))
                .map(|window| window.id);
            match marked_window {
                Some(compositor_window) => {
                    let restored_title = probe.target.known_title().or_else(|| {
                        probe
                            .titles_before
                            .get(&compositor_window)
                            .map(String::as_str)
                    });
                    if let Some(restored_title) = restored_title {
                        restore_title(access, &probe.target, restored_title);
                    }
                    self.paired.insert(compositor_window, probe.key);
                }
                None if now
                    .checked_duration_since(probe.sent_at)
                    .expect("a probe is sent at an earlier refresh")
                    >= PROBE_PATIENCE =>
                {
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
    compositor: &CompositorWindowState,
    process: i32,
) -> impl Iterator<Item = &CompositorWindow> {
    compositor
        .windows
        .values()
        .filter(move |window| window.pid == WindowPid::Reported(process))
}

fn window_title(compositor: &CompositorWindowState, compositor_window: u64) -> Option<&str> {
    match &compositor.windows.get(&compositor_window)?.title {
        WindowTitle::Titled(title) => Some(title),
        WindowTitle::Untitled => None,
    }
}
