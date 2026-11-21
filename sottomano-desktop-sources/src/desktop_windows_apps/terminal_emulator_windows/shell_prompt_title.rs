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
pub enum ShellTitle<'terminal> {
    IdlePrompt,
    RunningCommand(&'terminal str),
    ProgramTitle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptPresence {
    AtPrompt,
    AwayFromPrompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForegroundProgram<'terminal> {
    NoForegroundProgram,
    Running(&'terminal [String]),
}

pub struct ShellState<'terminal> {
    pub title: &'terminal str,
    pub cwd: &'terminal Path,
    pub home: &'terminal Path,
    pub prompt_presence: PromptPresence,
    pub reported_command_line: &'terminal str,
    pub foreground_program: ForegroundProgram<'terminal>,
}

impl ShellTitleFormat {
    pub fn read<'terminal>(self, shell: &ShellState<'terminal>) -> ShellTitle<'terminal> {
        match self {
            ShellTitleFormat::Nushell => read_nushell_title(shell),
            ShellTitleFormat::AnyShell => read_any_shell_title(shell),
        }
    }
}

fn read_nushell_title<'terminal>(shell: &ShellState<'terminal>) -> ShellTitle<'terminal> {
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

fn read_any_shell_title<'terminal>(shell: &ShellState<'terminal>) -> ShellTitle<'terminal> {
    if shell.prompt_presence == PromptPresence::AtPrompt
        || shell.foreground_program == ForegroundProgram::NoForegroundProgram
    {
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

    const FOREGROUND_COMMAND: &[String] = &[];

    fn shell_state<'terminal>(
        title: &'terminal str,
        prompt_presence: PromptPresence,
        reported: &'terminal str,
    ) -> ShellState<'terminal> {
        ShellState {
            title,
            cwd: Path::new("/home/user/proj"),
            home: Path::new("/home/user"),
            prompt_presence,
            reported_command_line: reported,
            foreground_program: ForegroundProgram::Running(FOREGROUND_COMMAND),
        }
    }

    #[test]
    fn nushell_titles_name_the_prompt_or_the_typed_command() {
        let read = |title| {
            ShellTitleFormat::Nushell.read(&shell_state(title, PromptPresence::AwayFromPrompt, ""))
        };
        assert_eq!(read("~/proj"), ShellTitle::IdlePrompt);
        assert_eq!(read("~/proj> cargo"), ShellTitle::RunningCommand("cargo"));
        assert_eq!(read("✳ Session"), ShellTitle::ProgramTitle);
        assert_eq!(read("~/project> cargo"), ShellTitle::ProgramTitle);
    }

    #[test]
    fn integrated_shells_report_the_prompt_and_command_line() {
        let read = |title, prompt_presence, reported| {
            ShellTitleFormat::AnyShell.read(&shell_state(title, prompt_presence, reported))
        };
        assert_eq!(
            read("…/a/b/c", PromptPresence::AtPrompt, ""),
            ShellTitle::IdlePrompt
        );
        assert_eq!(
            read("cargo build", PromptPresence::AwayFromPrompt, "cargo build"),
            ShellTitle::RunningCommand("cargo build")
        );
        assert_eq!(
            read("hx src/main.rs", PromptPresence::AwayFromPrompt, "hx"),
            ShellTitle::ProgramTitle
        );
    }

    #[test]
    fn a_shell_without_integration_is_idle_when_nothing_else_runs_in_the_foreground() {
        let idle = ShellState {
            foreground_program: ForegroundProgram::NoForegroundProgram,
            ..shell_state("user@host:~/proj", PromptPresence::AwayFromPrompt, "")
        };
        let busy = shell_state("user@host:~/proj", PromptPresence::AwayFromPrompt, "");
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
