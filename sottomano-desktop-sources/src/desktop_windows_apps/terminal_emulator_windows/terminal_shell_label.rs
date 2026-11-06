use std::path::Path;

use super::shell_prompt_title::{ShellState, ShellTitle, ShellTitleFormat};
use crate::desktop_windows_apps::project_directory_labels::project_relative_directory;

const SSH_VALUE_FLAGS: [&str; 20] = [
    "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o", "-p", "-Q",
    "-R", "-S", "-W", "-w",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalShellLabel {
    pub working_location: WorkingLocation,
    pub foreground_activity: ForegroundActivity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkingLocation {
    LocalDirectory(String),
    RemoteHost(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForegroundActivity {
    ShellPrompt {
        shell_name: String,
    },
    ShellCommand {
        command_line: String,
        program_name: Option<String>,
    },
    ProgramTitle {
        title: String,
        program_name: Option<String>,
    },
}

impl WorkingLocation {
    #[must_use]
    pub fn display_text(&self) -> &str {
        match self {
            WorkingLocation::LocalDirectory(directory) => directory,
            WorkingLocation::RemoteHost(host) => host,
        }
    }
}

impl ForegroundActivity {
    #[must_use]
    pub fn display_text(&self) -> &str {
        match self {
            ForegroundActivity::ShellPrompt { shell_name } => shell_name,
            ForegroundActivity::ShellCommand { command_line, .. } => command_line,
            ForegroundActivity::ProgramTitle { title, .. } => title,
        }
    }

    #[must_use]
    pub fn command_name(&self) -> Option<&str> {
        match self {
            ForegroundActivity::ShellPrompt { .. } => None,
            ForegroundActivity::ShellCommand {
                command_line,
                program_name,
            } => Some(program_name.as_deref().unwrap_or(command_line)),
            ForegroundActivity::ProgramTitle { program_name, .. } => program_name.as_deref(),
        }
    }
}

pub struct ActiveTerminalShell<'a> {
    pub shell: ShellState<'a>,
    pub shell_name: &'a str,
    pub foreground_program: Option<&'a [String]>,
}

impl ActiveTerminalShell<'_> {
    pub fn window_label(&self) -> TerminalShellLabel {
        let program_name = self.foreground_program.and_then(program_name);
        let title_format = ShellTitleFormat::from(self.shell_name);
        let foreground_activity = match title_format.read(&self.shell) {
            ShellTitle::IdlePrompt => ForegroundActivity::ShellPrompt {
                shell_name: self.shell_name.to_owned(),
            },
            ShellTitle::RunningCommand(command_line) => ForegroundActivity::ShellCommand {
                command_line: command_line.to_owned(),
                program_name,
            },
            ShellTitle::ProgramTitle => ForegroundActivity::ProgramTitle {
                title: self.shell.title.to_owned(),
                program_name,
            },
        };
        let remote_host = self
            .foreground_program
            .and_then(ssh_destination_arguments)
            .and_then(ssh_destination_host);
        let working_location = match remote_host {
            Some(host) => WorkingLocation::RemoteHost(host),
            None => WorkingLocation::LocalDirectory(project_relative_directory(
                self.shell.cwd,
                self.shell.home,
            )),
        };
        TerminalShellLabel {
            working_location,
            foreground_activity,
        }
    }
}

#[must_use]
pub fn program_name(cmdline: &[String]) -> Option<String> {
    let program = cmdline.first()?;
    Some(Path::new(program).file_name()?.to_str()?.to_owned())
}

fn ssh_destination_arguments(cmdline: &[String]) -> Option<&[String]> {
    let (_, arguments) = cmdline.split_first()?;
    match program_name(cmdline)?.as_str() {
        "ssh" => Some(arguments),
        "kitten" => (arguments.first()? == "ssh").then(|| &arguments[1..]),
        _ => None,
    }
}

fn ssh_destination_host(arguments: &[String]) -> Option<String> {
    let (argument, rest) = arguments.split_first()?;
    if SSH_VALUE_FLAGS.contains(&argument.as_str()) {
        return ssh_destination_host(rest.get(1..)?);
    }
    if argument.starts_with('-') {
        return ssh_destination_host(rest);
    }
    let host = argument.rsplit('@').next()?;
    Some(host.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/user";

    fn window_label(
        title: &str,
        cwd: &str,
        foreground_program: Option<&[String]>,
    ) -> TerminalShellLabel {
        ActiveTerminalShell {
            shell: ShellState {
                title,
                cwd: Path::new(cwd),
                home: Path::new(HOME),
                at_prompt: false,
                reported_command_line: "",
                has_foreground_program: foreground_program.is_some(),
            },
            shell_name: "nu",
            foreground_program,
        }
        .window_label()
    }

    fn cmdline(arguments: &[&str]) -> Vec<String> {
        arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect()
    }

    fn local_directory(path: &str) -> WorkingLocation {
        WorkingLocation::LocalDirectory(path.to_owned())
    }

    #[test]
    fn an_idle_nushell_prompt_is_titled_with_the_shell_name() {
        assert_eq!(
            window_label("~/Downloads", "/home/user/Downloads", None),
            TerminalShellLabel {
                working_location: local_directory("~/Downloads"),
                foreground_activity: ForegroundActivity::ShellPrompt {
                    shell_name: "nu".to_owned(),
                },
            }
        );
    }

    #[test]
    fn an_idle_prompt_is_titled_with_whichever_shell_the_window_runs() {
        let label = ActiveTerminalShell {
            shell: ShellState {
                title: "user@host:/srv",
                cwd: Path::new("/srv"),
                home: Path::new(HOME),
                at_prompt: false,
                reported_command_line: "",
                has_foreground_program: false,
            },
            shell_name: "xonsh",
            foreground_program: None,
        }
        .window_label();
        assert_eq!(
            label.foreground_activity,
            ForegroundActivity::ShellPrompt {
                shell_name: "xonsh".to_owned(),
            }
        );
    }

    #[test]
    fn a_nushell_command_title_names_the_typed_command() {
        assert_eq!(
            window_label("~/Downloads> sleep", "/home/user/Downloads", None),
            TerminalShellLabel {
                working_location: local_directory("~/Downloads"),
                foreground_activity: ForegroundActivity::ShellCommand {
                    command_line: "sleep".to_owned(),
                    program_name: None,
                },
            }
        );
    }

    #[test]
    fn a_program_title_is_kept_beside_the_program_name() {
        let claude = cmdline(&["/etc/profiles/per-user/user/bin/claude", "--resume"]);
        assert_eq!(
            window_label("✳ Session name", "/tmp", Some(&claude)),
            TerminalShellLabel {
                working_location: local_directory("/tmp"),
                foreground_activity: ForegroundActivity::ProgramTitle {
                    title: "✳ Session name".to_owned(),
                    program_name: Some("claude".to_owned()),
                },
            }
        );
    }

    #[test]
    fn ssh_sessions_are_located_on_their_remote_host() {
        let ssh = cmdline(&["ssh", "-p", "2222", "-A", "admin@builder"]);
        assert_eq!(
            window_label("~> ssh", HOME, Some(&ssh)).working_location,
            WorkingLocation::RemoteHost("builder".to_owned())
        );
        let kitten = cmdline(&["/usr/bin/kitten", "ssh", "builder"]);
        assert_eq!(
            window_label("~/proj> cargo", HOME, Some(&kitten)).working_location,
            WorkingLocation::RemoteHost("builder".to_owned())
        );
    }
}
