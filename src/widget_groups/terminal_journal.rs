use iced::Task;

use crate::{config::Config, data::source::niri, message_subscriptions::blocking};

#[derive(Debug, Clone)]
pub struct TerminalJournal {
    command: Vec<String>,
}

impl TerminalJournal {
    pub fn following(config: &Config, journalctl_args: Vec<String>) -> Self {
        Self {
            command: config
                .terminal_command_prefix
                .iter()
                .cloned()
                .chain(["journalctl".to_string()])
                .chain(journalctl_args)
                .collect(),
        }
    }

    pub fn open<M: Send + 'static>(self) -> Task<M> {
        let command = self.command;
        blocking("journal log terminal", move || {
            niri::send_action(&niri_ipc::Action::Spawn { command });
        })
        .discard()
    }
}
