use std::borrow::Cow;

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
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widget_groups::{
        journal_events_group::{
            JournalEventsInteraction, JournalScopeGroup, JournalTarget, shown_while_alerting,
        },
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
        TableRecord::with_cells(
            ValueCell::Time {
                at: self.at.into(),
                format: LocalizedTimeFormat::TimeOfDay,
            },
            [self.unit.name().into(), killer.into()],
        )
        .level(self.severity)
    }
}

impl JournalScopeGroup {
    pub(super) fn oom_kill_log_item(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Option<GroupItem<'_, JournalEventsInteraction>> {
        let locale = config.calendar.locale;
        let kills = self.oom_kills.display()?;
        Some(
            GroupItem::reading(
                kills,
                JournalTarget::OomKills(self.unit_scope),
                clock,
                locale,
            )
            .on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                    config,
                    OomKillEntry::follow_log_args(),
                )),
            ),
        )
    }
}

impl GroupItemReading for Vec<OomKillEntry> {
    fn name(&self) -> Cow<'static, str> {
        "OOM kills".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn badge(&self) -> Option<usize> {
        Some(self.len())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or_default()
            .into()
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            SectionTitle::captioned(
                "OOM kills",
                "Units whose processes were killed for lack of memory in the last hour.",
            ),
            headed_table(
                [
                    ColumnHeading::titled("Unit").align_x(Horizontal::Left),
                    ColumnHeading::titled("Killed by").align_x(Horizontal::Left),
                ],
                self.iter(),
            ),
        )]
    }

    fn panel_visibility(&self) -> PanelVisibility {
        shown_while_alerting(self.iter().map(|entry| entry.severity))
    }
}
