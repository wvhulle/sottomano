use iced::Task;

use crate::{config::Config, data::source::niri, message_subscriptions::blocking};

#[derive(Debug, Clone)]
pub struct TerminalCommand {
    command: Vec<String>,
}

impl TerminalCommand {
    pub fn running(config: &Config, program_and_args: impl IntoIterator<Item = String>) -> Self {
        Self {
            command: config
                .terminal_command_prefix
                .iter()
                .cloned()
                .chain(program_and_args)
                .collect(),
        }
    }

    pub fn open<M: Send + 'static>(self) -> Task<M> {
        let command = self.command;
        blocking("terminal command", move || {
            niri::send_action(&niri_ipc::Action::Spawn { command });
        })
        .discard()
    }
}
