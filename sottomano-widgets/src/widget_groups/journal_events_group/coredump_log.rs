use iced::alignment::Horizontal;
use sottomano_config::Config;
use sottomano_sources::systemd_units_journal::journal_coredumps::CoredumpEntry;

use crate::{
    clock::Clock,
    widget_groups::{
        journal_events_group::{JournalEventsGroup, JournalEventsMessage, JournalTarget},
        terminal_command::TerminalCommand,
    },
    widgets::{
        bar_widget::{BarWidget, Glyph},
        group_item::GroupItem,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::ValueCell,
        widget_state::WidgetState,
    },
};

impl IntoTableRecord<4> for &CoredumpEntry {
    fn into_table_record(self) -> TableRecord<4> {
        TableRecord::new(
            ValueCell::Time {
                at: self.at.into(),
                format: LocalizedTimeFormat::TimeOfDay,
            },
            [
                self.command.clone().into(),
                self.signal.to_string().into(),
                self.pid.to_string().into(),
                self.unit
                    .as_ref()
                    .map_or(ValueCell::Blank, |unit| unit.name().into()),
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
        Phosphor::Bug(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.len()).ok()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        self.iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Process").align_x(Horizontal::Left),
                ColumnHeading::titled("Signal").align_x(Horizontal::Left),
                ColumnHeading::titled("PID"),
                ColumnHeading::titled("Unit").align_x(Horizontal::Left),
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
