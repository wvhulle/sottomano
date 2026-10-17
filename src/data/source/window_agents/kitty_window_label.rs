use std::path::Path;

use super::shell_title::{ShellState, ShellTitle, ShellTitleFormat};
use crate::data::source::project_directory::project_relative_directory;

const SSH_VALUE_FLAGS: [&str; 20] = [
    "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o", "-p", "-Q",
    "-R", "-S", "-W", "-w",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyWindowLabel {
    pub working_location: WorkingLocation,
    pub foreground_activity: Option<ForegroundActivity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkingLocation {
    LocalDirectory(String),
    RemoteHost(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForegroundActivity {
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
    pub fn display_text(&self) -> &str {
        match self {
            WorkingLocation::LocalDirectory(directory) => directory,
            WorkingLocation::RemoteHost(host) => host,
        }
    }
}

impl ForegroundActivity {
    pub fn display_text(&self) -> &str {
        match self {
            ForegroundActivity::ShellCommand { command_line, .. } => command_line,
            ForegroundActivity::ProgramTitle { title, .. } => title,
        }
    }

    pub fn command_name(&self) -> Option<&str> {
        match self {
            ForegroundActivity::ShellCommand {
                command_line,
                program_name,
            } => Some(program_name.as_deref().unwrap_or(command_line)),
            ForegroundActivity::ProgramTitle { program_name, .. } => program_name.as_deref(),
        }
    }
}

pub struct ActiveKittyWindow<'a> {
    pub shell: ShellState<'a>,
    pub title_format: ShellTitleFormat,
    pub foreground_program: Option<&'a [String]>,
}

impl ActiveKittyWindow<'_> {
    pub fn window_label(&self) -> KittyWindowLabel {
        let program_name = self.foreground_program.and_then(program_name);
        let foreground_activity = match self.title_format.read(&self.shell) {
            ShellTitle::IdlePrompt => None,
            ShellTitle::RunningCommand(command_line) => Some(ForegroundActivity::ShellCommand {
                command_line: command_line.to_owned(),
                program_name,
            }),
            ShellTitle::ProgramTitle => Some(ForegroundActivity::ProgramTitle {
                title: self.shell.title.to_owned(),
                program_name,
            }),
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
        KittyWindowLabel {
            working_location,
            foreground_activity,
        }
    }
}

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
    ) -> KittyWindowLabel {
        ActiveKittyWindow {
            shell: ShellState {
                title,
                cwd: Path::new(cwd),
                home: Path::new(HOME),
                at_prompt: false,
                reported_command_line: "",
            },
            title_format: ShellTitleFormat::Nushell,
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
    fn an_idle_nushell_prompt_has_no_foreground_activity() {
        assert_eq!(
            window_label("~/Downloads", "/home/user/Downloads", None),
            KittyWindowLabel {
                working_location: local_directory("~/Downloads"),
                foreground_activity: None,
            }
        );
    }

    #[test]
    fn a_nushell_command_title_names_the_typed_command() {
        assert_eq!(
            window_label("~/Downloads> sleep", "/home/user/Downloads", None),
            KittyWindowLabel {
                working_location: local_directory("~/Downloads"),
                foreground_activity: Some(ForegroundActivity::ShellCommand {
                    command_line: "sleep".to_owned(),
                    program_name: None,
                }),
            }
        );
    }

    #[test]
    fn a_program_title_is_kept_beside_the_program_name() {
        let claude = cmdline(&["/etc/profiles/per-user/user/bin/claude", "--resume"]);
        assert_eq!(
            window_label("✳ Session name", "/tmp", Some(&claude)),
            KittyWindowLabel {
                working_location: local_directory("/tmp"),
                foreground_activity: Some(ForegroundActivity::ProgramTitle {
                    title: "✳ Session name".to_owned(),
                    program_name: Some("claude".to_owned()),
                }),
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
