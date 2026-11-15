use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::journal_coredumps::CoredumpEntry;
use sottomano_toml_config::Config;

use crate::{
    clock::Clock,
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
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

impl IntoTableRecord<4> for &CoredumpEntry {
    fn into_table_record(self) -> TableRecord<4> {
        TableRecord::with_cells(
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

impl JournalScopeGroup {
    pub(super) fn coredump_log_item(
        &self,
        config: &Config,
        clock: Clock,
    ) -> Option<GroupItem<'_, JournalEventsInteraction>> {
        let coredumps = self.coredumps.reading_row(
            IconGlyph::default(),
            |snapshot| coredump_log_reading_row(snapshot),
            clock,
        )?;
        Some(
            GroupItem::reading(coredumps)
                .on_press(
                    TerminalCommand::JOURNAL_PRESS_HINT,
                    JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                        config,
                        CoredumpEntry::follow_log_args(),
                    )),
                )
                .with_hover_card(Box::new(move |_| {
                    self.coredumps
                        .sections_described_by(|snapshot| coredump_log_hover_card(snapshot))
                })),
        )
    }
}

pub(super) fn coredump_log_state(entries: &[CoredumpEntry]) -> ReadingDisplayState {
    entries
        .iter()
        .map(|entry| entry.severity)
        .max()
        .unwrap_or_default()
        .into()
}

pub(super) fn coredump_log_panel_visibility(entries: &[CoredumpEntry]) -> PanelVisibility {
    shown_while_alerting(entries.iter().map(|entry| entry.severity))
}

pub(super) fn coredump_log_reading_row(entries: &[CoredumpEntry]) -> ReadingRow<'_> {
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Unlabeled,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Counted(entries.len()),
        },
        panel_visibility: coredump_log_panel_visibility(entries),
        ..ReadingRow::glyph_only(
            "Coredumps",
            Phosphor::Bug(Weight::Regular).into(),
            coredump_log_state(entries),
            Box::new(move |_| coredump_log_hover_card(entries)),
        )
    }
}

pub(super) fn coredump_log_hover_card(entries: &[CoredumpEntry]) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Coredumps",
            "Programs that crashed in the last hour, as recorded by systemd-coredump.",
        ),
        headed_table(
            [
                ColumnHeading::titled("Process").align_x(Horizontal::Left),
                ColumnHeading::titled("Signal").align_x(Horizontal::Left),
                ColumnHeading::titled("PID"),
                ColumnHeading::titled("Unit").align_x(Horizontal::Left),
            ],
            entries.iter(),
        ),
    )]
}
