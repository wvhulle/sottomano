use crate::{
    config::Config,
    data::{
        notifications::model::ActionTint,
        source::coredumps::{CoredumpEntry, CoredumpInspection},
    },
    widget_groups::{
        systemd_manager_group::SystemdManagerMessage, terminal_command::TerminalCommand,
    },
    widgets::ongoing_toast::OngoingToastActionButton,
};

impl CoredumpEntry {
    pub fn labelled_fields(&self) -> impl Iterator<Item = String> {
        let stored_file = self
            .stored_file
            .as_ref()
            .map(|file| format!("<b>Core</b> <tt>{}</tt>", file.display()));
        [
            format!("<b>Signal</b> {}", self.signal),
            format!(
                "<b>Process</b> {} · PID {}",
                self.executable.display(),
                self.pid
            ),
        ]
        .into_iter()
        .chain(stored_file)
    }

    pub fn action_buttons(
        &self,
        config: &Config,
    ) -> impl Iterator<Item = OngoingToastActionButton<SystemdManagerMessage>> {
        let inspect = |inspection| {
            SystemdManagerMessage::InspectCoredump(TerminalCommand::running(
                config,
                self.coredumpctl_args(inspection),
            ))
        };
        let copy_path = self
            .stored_file
            .clone()
            .map(|file| OngoingToastActionButton {
                button_label: "Copy path".to_owned(),
                tint: ActionTint::Inspect,
                on_button_pressed: SystemdManagerMessage::CopyToClipboard(
                    file.display().to_string(),
                ),
            });
        [
            OngoingToastActionButton {
                button_label: "Info".to_owned(),
                tint: ActionTint::Inspect,
                on_button_pressed: inspect(CoredumpInspection::Info),
            },
            OngoingToastActionButton {
                button_label: "Debug".to_owned(),
                tint: ActionTint::Inspect,
                on_button_pressed: inspect(CoredumpInspection::Debug),
            },
        ]
        .into_iter()
        .chain(copy_path)
    }
}
