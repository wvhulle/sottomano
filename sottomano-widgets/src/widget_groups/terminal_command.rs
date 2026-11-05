use iced::Task;
use sottomano_desktop_sources::{
    blocking_background_work::blocking, desktop_windows_apps::niri_action_requests,
};
use sottomano_toml_config::Config;

#[derive(Debug, Clone)]
pub struct TerminalCommand {
    command: Vec<String>,
}

impl TerminalCommand {
    pub const JOURNAL_PRESS_HINT: &str = "open the journal in a terminal";

    #[must_use]
    pub fn journal(config: &Config, journalctl_args: Vec<String>) -> Self {
        Self::running(
            config,
            ["journalctl".to_string()]
                .into_iter()
                .chain(journalctl_args),
        )
    }

    pub fn running(config: &Config, program_and_args: impl IntoIterator<Item = String>) -> Self {
        Self {
            command: config
                .launched_programs
                .terminal_command_prefix
                .iter()
                .cloned()
                .chain(program_and_args)
                .collect(),
        }
    }

    pub fn open<M: Send + 'static>(self) -> Task<M> {
        let command = self.command;
        Task::future(blocking("terminal command", move || {
            niri_action_requests::send_action_or_warn(&niri_ipc::Action::Spawn { command });
        }))
        .discard()
    }
}
