use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::journal_coredumps::CoredumpEntry;
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::{GroupItemReading, PanelVisibility},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::{
        journal_events_group::{JournalEventsGroup, JournalEventsInteraction, JournalTarget},
        terminal_command::TerminalCommand,
    },
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
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
    ) -> Option<GroupItem<'_, JournalEventsInteraction>> {
        let coredumps = self.coredumps.display()?;
        Some(
            GroupItem::reading(coredumps, JournalTarget::Coredumps).on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                    config,
                    CoredumpEntry::follow_log_args(),
                )),
            ),
        )
    }
}

impl GroupItemReading for Vec<CoredumpEntry> {
    fn name(&self) -> &'static str {
        "Coredumps"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Bug(Weight::Regular).into()
    }

    fn badge(&self) -> Option<u32> {
        u32::try_from(self.len()).ok()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::column_table(
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

    fn panel_visibility(&self) -> PanelVisibility {
        if self.is_empty() {
            PanelVisibility::Hidden
        } else {
            PanelVisibility::Shown
        }
    }
}
