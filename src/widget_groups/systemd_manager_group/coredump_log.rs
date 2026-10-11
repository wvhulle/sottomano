use crate::{
    config::Config,
    data::{
        notifications::model::ActionTint,
        source::coredumps::{CoredumpEntry, CoredumpInspection, Coredumps},
    },
    widget_groups::{
        systemd_manager_group::{SystemdManagerMessage, SystemdTarget},
        terminal_command::TerminalCommand,
        terminal_journal::TerminalJournal,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::TimePart,
        ongoing_toast::OngoingToastActionButton,
        phosphor::Phosphor,
        tooltip_body::{Cell, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

impl CoredumpEntry {
    fn table_record(&self) -> TableRecord<4> {
        TableRecord::new(
            Cell::Time {
                at: self.at.into(),
                part: TimePart::TimeOfDay,
            },
            [
                Some(self.command.clone().into()),
                Some(self.signal.clone().into()),
                Some(self.pid.clone().into()),
                self.unit.as_ref().map(|unit| unit.name().into()),
            ],
        )
        .level(self.severity)
    }
}

impl CoredumpEntry {
    pub fn labelled_fields(&self) -> impl Iterator<Item = String> {
        let stored_file = self
            .stored_file
            .as_ref()
            .map(|file| format!("<b>Core</b> <tt>{}</tt>", file.display()));
        [
            format!("<b>Signal</b> {}", self.signal),
            format!("<b>Process</b> {} · PID {}", self.executable, self.pid),
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

impl Coredumps {
    pub fn group_item(&self, config: &Config) -> GroupItem<'_, SystemdManagerMessage> {
        GroupItem::reading(self, SystemdTarget::Coredumps).on_press(
            TerminalJournal::PRESS_HINT,
            SystemdManagerMessage::OpenLog(TerminalJournal::following(
                config,
                Coredumps::follow_log_args(),
            )),
        )
    }
}

impl BarWidget for Coredumps {
    fn name(&self) -> &'static str {
        "Coredumps"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Bug(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.entries.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.entries
            .iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::column_table(
            (
                "Coredumps",
                "Programs that crashed in the last hour, as recorded by systemd-coredump.",
            ),
            ["Process", "Signal", "PID", "Unit"],
            self.entries.iter().map(CoredumpEntry::table_record),
        )])
    }
}
