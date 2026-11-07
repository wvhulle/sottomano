use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::journal_oom_kills::{
    OomKillEntry, OomKiller,
};
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
        journal_events_group::{JournalEventsGroup, JournalEventsMessage, JournalTarget},
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

impl IntoTableRecord<2> for &OomKillEntry {
    fn into_table_record(self) -> TableRecord<2> {
        let killer = match self.killer {
            OomKiller::Kernel => "kernel",
            OomKiller::Oomd => "systemd-oomd",
        };
        TableRecord::new(
            ValueCell::Time {
                at: self.at.into(),
                format: LocalizedTimeFormat::TimeOfDay,
            },
            [self.unit.name().into(), killer.into()],
        )
        .level(self.severity)
    }
}

impl JournalEventsGroup {
    pub(super) fn oom_kill_log_item(
        &self,
        config: &Config,
    ) -> Option<GroupItem<'_, JournalEventsMessage>> {
        let kills = self.oom_kills.display()?;
        Some(GroupItem::reading(kills, JournalTarget::OomKills).on_press(
            TerminalCommand::JOURNAL_PRESS_HINT,
            JournalEventsMessage::OpenLog(TerminalCommand::journal(
                config,
                OomKillEntry::follow_log_args(),
            )),
        ))
    }
}

impl GroupItemReading for Vec<OomKillEntry> {
    fn name(&self) -> &'static str {
        "OOM kills"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Memory(Weight::Regular).into()
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
                ColumnHeading::titled("Unit").align_x(Horizontal::Left),
                ColumnHeading::titled("Killed by").align_x(Horizontal::Left),
            ],
            self.iter(),
        )
        .titled((
            "OOM kills",
            "Units whose processes were killed for lack of memory in the last hour.",
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
