use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use sottomano_compositor_model::{
    compositor_test_windows::unplaced_compositor_window,
    compositor_window_facts::{WindowAppId, WindowPid, WindowTitle},
    compositor_window_state::{CompositorWindow, CompositorWindowState},
};
use sottomano_toml_config::output_windows::{TerminalAgentUse, WindowAgentsConfig};

use super::{
    TerminalAgent, TerminalAgentAccess, TerminalLabelSource, TerminalWindow, TerminalWindowKey,
    TitleProbeTarget, title_probe_pairing::Pairing,
};

const GHOSTTY_PID: i32 = 700;

#[derive(Default)]
struct ScriptedTerminal {
    listed: Vec<TerminalWindow>,
    held_titles: Vec<(PathBuf, String)>,
    yielded_titles: Vec<(PathBuf, String)>,
}

fn probe_target_pty(target: &TitleProbeTarget) -> PathBuf {
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
        _compositor_window_count: usize,
    ) -> io::Result<Vec<TerminalWindow>> {
        Ok(self.listed.clone())
    }

    fn hold_title(&mut self, target: &TitleProbeTarget, marker: &str) -> io::Result<()> {
        self.held_titles
            .push((probe_target_pty(target), marker.to_owned()));
        Ok(())
    }

    fn yield_title(&mut self, target: &TitleProbeTarget, title: &str) -> io::Result<()> {
        self.yielded_titles
            .push((probe_target_pty(target), title.to_owned()));
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
        label_source: TerminalLabelSource::Unlabelled,
    }
}

fn ghostty_windows(titles: &[(u64, &str)]) -> CompositorWindowState {
    let ghostty_windows = titles.iter().map(|&(window_id, title)| CompositorWindow {
        pid: WindowPid::Reported(GHOSTTY_PID),
        app_id: WindowAppId::Reported("com.mitchellh.ghostty".to_owned()),
        title: WindowTitle::Titled(title.to_owned()),
        ..unplaced_compositor_window(window_id)
    });
    CompositorWindowState::indexed_by_id([], ghostty_windows)
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
    let processes = pairing().refresh(&ghostty_windows(&[(1, "~")]), &mut terminal, Instant::now());
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
    let compositor = ghostty_windows(&[(1, "~/a"), (2, "~/b")]);
    pairing.refresh(&compositor, &mut terminal, started);
    let later = started + Duration::from_secs(3);
    let processes = pairing.refresh(&compositor, &mut terminal, later);
    pairing.refresh(&compositor, &mut terminal, later);
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
            pty_shells: TerminalAgentUse::Disabled,
            ..WindowAgentsConfig::default()
        },
    );
    let processes = pairing.refresh(&ghostty_windows(&[(1, "~")]), &mut terminal, Instant::now());
    assert_eq!(processes, super::WindowProcesses::default());
    assert!(pairing.paired.is_empty());
}
