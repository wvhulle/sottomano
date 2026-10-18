use iced::alignment::Horizontal;
use sottomano_config::Config;
use sottomano_sources::coredumps::CoredumpEntry;

use crate::{
    widget_groups::{
        journal_events_group::{JournalEventsGroup, JournalEventsMessage, JournalTarget},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        localized_time::LocalizedTimeFormat,
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TableRow, TooltipBody},
        widget_state::WidgetState,
    },
};

impl TableRow<4> for &CoredumpEntry {
    fn table_record(self) -> TableRecord<4> {
        TableRecord::new(
            Cell::Time {
                at: self.at.into(),
                format: LocalizedTimeFormat::TimeOfDay,
            },
            [
                Some(self.command.clone().into()),
                Some(self.signal.to_string().into()),
                Some(self.pid.to_string().into()),
                self.unit.as_ref().map(|unit| unit.name().into()),
            ],
        )
        .level(self.severity)
    }
}

impl JournalEventsGroup {
    pub(super) fn coredump_log_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, JournalEventsMessage>> {
        let coredumps = self.coredumps.display()?;
        Some(
            GroupItem::reading(coredumps, JournalTarget::Coredumps).on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                JournalEventsMessage::OpenLog(TerminalCommand::journal(
                    config,
                    CoredumpEntry::follow_log_args(),
                )),
            ),
        )
    }
}

impl BarWidget for Vec<CoredumpEntry> {
    fn name(&self) -> &'static str {
        "Coredumps"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Bug(self.state().weight()).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.len()).ok()
    }

    fn state(&self) -> WidgetState {
        self.iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::new("Process").align_x(Horizontal::Left),
                ColumnHeading::new("Signal").align_x(Horizontal::Left),
                ColumnHeading::new("PID"),
                ColumnHeading::new("Unit").align_x(Horizontal::Left),
            ],
            self.iter(),
        )
        .titled((
            "Coredumps",
            "Programs that crashed in the last hour, as recorded by systemd-coredump.",
        ))])
    }

    fn visible_in_bar(&self) -> bool {
        !self.is_empty()
    }
}
