use std::path::Path;

use crate::data::source::project_directory::home_abbreviated_path;

const NUSHELL_COMMAND_SEPARATOR: &str = "> ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellTitleFormat {
    Nushell,
    KittyIntegration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedShell;

impl TryFrom<&str> for ShellTitleFormat {
    type Error = UnsupportedShell;

    fn try_from(shell_name: &str) -> Result<Self, Self::Error> {
        match shell_name {
            "nu" => Ok(ShellTitleFormat::Nushell),
            "bash" | "zsh" | "fish" => Ok(ShellTitleFormat::KittyIntegration),
            _ => Err(UnsupportedShell),
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
}

impl ShellTitleFormat {
    pub fn read<'a>(self, shell: &ShellState<'a>) -> ShellTitle<'a> {
        match self {
            ShellTitleFormat::Nushell => read_nushell_title(shell),
            ShellTitleFormat::KittyIntegration => read_kitty_integration_title(shell),
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

fn read_kitty_integration_title<'a>(shell: &ShellState<'a>) -> ShellTitle<'a> {
    if shell.at_prompt {
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
    fn kitty_integrated_shells_report_the_prompt_and_command_line() {
        let read = |title, at_prompt, reported| {
            ShellTitleFormat::KittyIntegration.read(&shell_state(title, at_prompt, reported))
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
    fn only_known_shells_have_a_title_format() {
        assert_eq!(
            ShellTitleFormat::try_from("nu"),
            Ok(ShellTitleFormat::Nushell)
        );
        assert_eq!(
            ShellTitleFormat::try_from("zsh"),
            Ok(ShellTitleFormat::KittyIntegration)
        );
        assert_eq!(ShellTitleFormat::try_from("xonsh"), Err(UnsupportedShell));
    }
}
