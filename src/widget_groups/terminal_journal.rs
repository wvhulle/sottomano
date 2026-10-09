use iced::Task;

use crate::{config::Config, widget_groups::terminal_command::TerminalCommand};

#[derive(Debug, Clone)]
pub struct TerminalJournal {
    terminal: TerminalCommand,
}

impl TerminalJournal {
    pub const PRESS_HINT: &str = "open the journal in a terminal";

    pub fn following(config: &Config, journalctl_args: Vec<String>) -> Self {
        Self {
            terminal: TerminalCommand::running(
                config,
                ["journalctl".to_string()]
                    .into_iter()
                    .chain(journalctl_args),
            ),
        }
    }

    pub fn open<M: Send + 'static>(self) -> Task<M> {
        self.terminal.open()
    }
}
