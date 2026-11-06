use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use procfs::process::Process as Proc;
use rustix::{fs::stat as file_status, process::getuid};

use super::{
    TerminalLabelReading, TerminalWindow, TerminalWindowKey, TitleProbeTarget,
    shell_prompt_title::ShellState,
    terminal_shell_label::{ActiveTerminalShell, TerminalShellLabel, program_name},
};

const PTY_MAJOR_NUMBERS: RangeInclusive<i32> = 136..=143;
const PTY_MINOR_COUNT: i32 = 256;
const LOGIN_SHELL_PREFIX: char = '-';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellForeground {
    AtPrompt,
    Running(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSession {
    pub shell_pid: i32,
    pub pty_path: PathBuf,
    pub pty_number: u32,
    pub shell_command: Vec<String>,
    pub cwd: PathBuf,
    pub foreground: ShellForeground,
}

impl ShellSession {
    pub fn terminal_label(&self, niri_title: &str, home: &Path) -> Option<TerminalShellLabel> {
        let shell_name = program_name(&self.shell_command)?;
        let foreground_program = match &self.foreground {
            ShellForeground::AtPrompt => None,
            ShellForeground::Running(command) => Some(command.as_slice()),
        };
        let active_shell = ActiveTerminalShell {
            shell: ShellState {
                title: niri_title,
                cwd: &self.cwd,
                home,
                at_prompt: foreground_program.is_none(),
                reported_command_line: "",
                has_foreground_program: foreground_program.is_some(),
            },
            shell_name: shell_name.trim_start_matches(LOGIN_SHELL_PREFIX),
            foreground_program,
        };
        Some(active_shell.window_label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcessSessionFacts {
    pid: i32,
    parent_pid: i32,
    session_id: i32,
}

impl ProcessSessionFacts {
    fn leads_a_session_under(self, terminal_pid: i32) -> bool {
        self.parent_pid == terminal_pid && self.session_id == self.pid
    }
}

fn pty_number_of(tty_major: i32, tty_minor: i32) -> Option<u32> {
    if !PTY_MAJOR_NUMBERS.contains(&tty_major) {
        return None;
    }
    u32::try_from((tty_major - PTY_MAJOR_NUMBERS.start()) * PTY_MINOR_COUNT + tty_minor).ok()
}

pub fn child_pids(process: i32) -> Vec<i32> {
    let Ok(parent) = Proc::new(process) else {
        return Vec::new();
    };
    let Ok(tasks) = parent.tasks() else {
        return Vec::new();
    };
    tasks
        .filter_map(Result::ok)
        .filter_map(|task| task.children().ok())
        .flatten()
        .filter_map(|child| i32::try_from(child).ok())
        .collect()
}

pub fn shell_sessions(terminal_pid: i32) -> Vec<ShellSession> {
    let our_uid = getuid().as_raw();
    child_pids(terminal_pid)
        .into_iter()
        .filter_map(|child| read_shell_session(child, terminal_pid, our_uid))
        .collect()
}

fn read_shell_session(pid: i32, terminal_pid: i32, our_uid: u32) -> Option<ShellSession> {
    let process = Proc::new(pid).ok()?;
    let stat = process.stat().ok()?;
    let session_facts = ProcessSessionFacts {
        pid,
        parent_pid: stat.ppid,
        session_id: stat.session,
    };
    if !session_facts.leads_a_session_under(terminal_pid) {
        return None;
    }
    let (tty_major, tty_minor) = stat.tty_nr();
    let pty_number = pty_number_of(tty_major, tty_minor)?;
    let pty_path = PathBuf::from(format!("/dev/pts/{pty_number}"));
    if file_status(&pty_path).ok()?.st_uid != our_uid {
        return None;
    }
    let foreground = if stat.tpgid <= 0 || stat.tpgid == stat.pgrp {
        ShellForeground::AtPrompt
    } else {
        Proc::new(stat.tpgid)
            .and_then(|leader| leader.cmdline())
            .map_or(ShellForeground::AtPrompt, ShellForeground::Running)
    };
    Some(ShellSession {
        shell_pid: pid,
        pty_path,
        pty_number,
        shell_command: process.cmdline().ok()?,
        cwd: process.cwd().ok()?,
        foreground,
    })
}

pub fn terminal_windows(terminal_pid: i32, niri_window_count: usize) -> Vec<TerminalWindow> {
    let sessions = shell_sessions(terminal_pid);
    if sessions.len() != niri_window_count {
        tracing::debug!(
            "terminal {terminal_pid} runs {} shells in {niri_window_count} windows, so tabs hide \
             some",
            sessions.len()
        );
        return Vec::new();
    }
    sessions
        .into_iter()
        .map(|session| TerminalWindow {
            key: TerminalWindowKey {
                process: terminal_pid,
                terminal_window: u64::from(session.pty_number),
            },
            probe_target: TitleProbeTarget::ShellPty {
                pty_path: session.pty_path.clone(),
            },
            shown_pid: session.shell_pid,
            pids: vec![session.shell_pid],
            label_reading: TerminalLabelReading::ShellSession(session),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_windows_apps::terminal_emulator_windows::terminal_shell_label::{
        ForegroundActivity, WorkingLocation,
    };

    fn nu_session(foreground: ShellForeground) -> ShellSession {
        ShellSession {
            shell_pid: 900,
            pty_path: PathBuf::from("/dev/pts/7"),
            pty_number: 7,
            shell_command: vec!["-nu".to_owned()],
            cwd: PathBuf::from("/home/user/Code/sottomano"),
            foreground,
        }
    }

    #[test]
    fn only_a_session_leader_spawned_by_the_terminal_counts_as_its_shell() {
        let shell = ProcessSessionFacts {
            pid: 900,
            parent_pid: 700,
            session_id: 900,
        };
        assert!(shell.leads_a_session_under(700));
        assert!(!shell.leads_a_session_under(701));
        let helper = ProcessSessionFacts {
            session_id: 899,
            ..shell
        };
        assert!(!helper.leads_a_session_under(700));
    }

    #[test]
    fn only_pseudo_terminals_have_a_pty_number() {
        assert_eq!(pty_number_of(136, 7), Some(7));
        assert_eq!(pty_number_of(137, 2), Some(258));
        assert_eq!(pty_number_of(4, 1), None);
        assert_eq!(pty_number_of(0, 0), None);
    }

    #[test]
    fn a_login_shell_at_its_prompt_is_labelled_with_its_directory() {
        let label = nu_session(ShellForeground::AtPrompt)
            .terminal_label("~/Code/sottomano", Path::new("/home/user"))
            .unwrap();
        assert_eq!(
            label.foreground_activity,
            ForegroundActivity::ShellPrompt {
                shell_name: "nu".to_owned(),
            }
        );
        assert!(matches!(
            label.working_location,
            WorkingLocation::LocalDirectory(_)
        ));
    }

    #[test]
    fn an_ssh_foreground_program_locates_the_window_on_its_host() {
        let ssh = vec!["ssh".to_owned(), "builder".to_owned()];
        let label = nu_session(ShellForeground::Running(ssh))
            .terminal_label("~> ssh builder", Path::new("/home/user"))
            .unwrap();
        assert_eq!(
            label.working_location,
            WorkingLocation::RemoteHost("builder".to_owned())
        );
    }
}
