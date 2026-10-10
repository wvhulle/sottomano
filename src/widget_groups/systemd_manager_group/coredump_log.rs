use crate::{
    config::Config,
    data::source::coredumps::{CoredumpEntry, Coredumps},
    widget_groups::{
        systemd_manager_group::{SystemdManagerMessage, SystemdTarget},
        terminal_journal::TerminalJournal,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::TimePart,
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
