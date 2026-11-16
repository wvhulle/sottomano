use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::journal_oom_kills::{
    OomKillEntry, OomKiller,
};
use sottomano_toml_config::Config;

use crate::{
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widget_groups::{
        journal_events_group::{JournalEventsInteraction, JournalScopeGroup, shown_while_alerting},
        terminal_command::TerminalCommand,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

impl From<&OomKillEntry> for TableRecord<2> {
    fn from(oom_kill_entry: &OomKillEntry) -> Self {
        let killer = match oom_kill_entry.killer {
            OomKiller::Kernel => "kernel",
            OomKiller::Oomd => "systemd-oomd",
        };
        TableRecord::with_cells(
            ValueCell::Time {
                at: oom_kill_entry.at.into(),
                format: LocalizedTimeFormat::TimeOfDay,
            },
            [oom_kill_entry.unit.name().into(), killer.into()],
        )
        .level(oom_kill_entry.severity)
    }
}

pub(super) const OOM_KILL_LOG_GLYPH: Phosphor = Phosphor::Memory(Weight::Regular);

impl JournalScopeGroup {
    pub(super) fn oom_kill_log_item<'a>(
        &'a self,
        config: &Config,
        oom_kill_row: ReadingRow<'a>,
    ) -> GroupItem<'a, JournalEventsInteraction> {
        GroupItem::reading(oom_kill_row)
            .on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                    config,
                    OomKillEntry::follow_log_args(),
                )),
            )
            .with_hover_card(Box::new(move |_| {
                self.oom_kills
                    .sections_described_by(|snapshot| oom_kill_log_hover_card(snapshot))
            }))
    }
}

pub(super) fn oom_kill_log_state(entries: &[OomKillEntry]) -> ReadingDisplayState {
    entries
        .iter()
        .map(|entry| entry.severity)
        .max()
        .unwrap_or_default()
        .into()
}

pub(super) fn oom_kill_log_panel_visibility(entries: &[OomKillEntry]) -> PanelVisibility {
    shown_while_alerting(entries.iter().map(|entry| entry.severity))
}

pub(super) fn oom_kill_log_reading_row(entries: &[OomKillEntry]) -> ReadingRow<'_> {
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Counted(entries.len()),
        },
        panel_visibility: oom_kill_log_panel_visibility(entries),
        ..ReadingRow::glyph_only(
            "OOM kills",
            OOM_KILL_LOG_GLYPH.into(),
            oom_kill_log_state(entries),
            Box::new(move |_| oom_kill_log_hover_card(entries)),
        )
    }
}

pub(super) fn oom_kill_log_hover_card(entries: &[OomKillEntry]) -> Vec<Section> {
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
            entries.iter(),
        ),
    )]
}
