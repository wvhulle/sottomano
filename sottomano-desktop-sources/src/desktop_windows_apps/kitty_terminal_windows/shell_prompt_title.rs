use std::path::Path;

use crate::desktop_windows_apps::project_directory_labels::home_abbreviated_path;

const NUSHELL_COMMAND_SEPARATOR: &str = "> ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellTitleFormat {
    Nushell,
    AnyShell,
}

impl From<&str> for ShellTitleFormat {
    fn from(shell_name: &str) -> Self {
        match shell_name {
            "nu" => ShellTitleFormat::Nushell,
            _ => ShellTitleFormat::AnyShell,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellTitle<'a> {
    IdlePrompt,
    RunningCommand(&'a str),
    ProgramTitle,
}

pub struct ShellState<'a> {
    pub title: &'a str,
    pub cwd: &'a Path,
    pub home: &'a Path,
    pub at_prompt: bool,
    pub reported_command_line: &'a str,
    pub has_foreground_program: bool,
}

impl ShellTitleFormat {
    pub fn read<'a>(self, shell: &ShellState<'a>) -> ShellTitle<'a> {
        match self {
            ShellTitleFormat::Nushell => read_nushell_title(shell),
            ShellTitleFormat::AnyShell => read_any_shell_title(shell),
        }
    }
}

fn read_nushell_title<'a>(shell: &ShellState<'a>) -> ShellTitle<'a> {
    let prompt_title = home_abbreviated_path(shell.cwd, shell.home);
    if shell.title == prompt_title {
        return ShellTitle::IdlePrompt;
    }
    let Some(rest) = shell.title.strip_prefix(prompt_title.as_str()) else {
        return ShellTitle::ProgramTitle;
    };
    match rest.strip_prefix(NUSHELL_COMMAND_SEPARATOR) {
        Some(command) => ShellTitle::RunningCommand(command),
        None => ShellTitle::ProgramTitle,
    }
}

fn read_any_shell_title<'a>(shell: &ShellState<'a>) -> ShellTitle<'a> {
    if shell.at_prompt || !shell.has_foreground_program {
        return ShellTitle::IdlePrompt;
    }
    if !shell.reported_command_line.is_empty() && shell.title == shell.reported_command_line {
        return ShellTitle::RunningCommand(shell.reported_command_line);
    }
    ShellTitle::ProgramTitle
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_state<'a>(title: &'a str, at_prompt: bool, reported: &'a str) -> ShellState<'a> {
        ShellState {
            title,
            cwd: Path::new("/home/user/proj"),
            home: Path::new("/home/user"),
            at_prompt,
            reported_command_line: reported,
            has_foreground_program: true,
        }
    }

    #[test]
    fn nushell_titles_name_the_prompt_or_the_typed_command() {
        let read = |title| ShellTitleFormat::Nushell.read(&shell_state(title, false, ""));
        assert_eq!(read("~/proj"), ShellTitle::IdlePrompt);
        assert_eq!(read("~/proj> cargo"), ShellTitle::RunningCommand("cargo"));
        assert_eq!(read("✳ Session"), ShellTitle::ProgramTitle);
        assert_eq!(read("~/project> cargo"), ShellTitle::ProgramTitle);
    }

    #[test]
    fn integrated_shells_report_the_prompt_and_command_line() {
        let read = |title, at_prompt, reported| {
            ShellTitleFormat::AnyShell.read(&shell_state(title, at_prompt, reported))
        };
        assert_eq!(read("…/a/b/c", true, ""), ShellTitle::IdlePrompt);
        assert_eq!(
            read("cargo build", false, "cargo build"),
            ShellTitle::RunningCommand("cargo build")
        );
        assert_eq!(
            read("hx src/main.rs", false, "hx"),
            ShellTitle::ProgramTitle
        );
    }

    #[test]
    fn a_shell_without_integration_is_idle_when_nothing_else_runs_in_the_foreground() {
        let idle = ShellState {
            has_foreground_program: false,
            ..shell_state("user@host:~/proj", false, "")
        };
        let busy = shell_state("user@host:~/proj", false, "");
        assert_eq!(
            ShellTitleFormat::AnyShell.read(&idle),
            ShellTitle::IdlePrompt
        );
        assert_eq!(
            ShellTitleFormat::AnyShell.read(&busy),
            ShellTitle::ProgramTitle
        );
    }

    #[test]
    fn only_nushell_has_its_own_title_format() {
        assert_eq!(ShellTitleFormat::from("nu"), ShellTitleFormat::Nushell);
        assert_eq!(ShellTitleFormat::from("zsh"), ShellTitleFormat::AnyShell);
        assert_eq!(ShellTitleFormat::from("xonsh"), ShellTitleFormat::AnyShell);
    }
}
