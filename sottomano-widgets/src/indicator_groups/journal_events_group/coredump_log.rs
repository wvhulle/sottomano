use iced::alignment::Horizontal;
use sottomano_desktop_sources::systemd_units_journal::journal_coredumps::{
    CoredumpEntry, CoredumpOwner,
};
use sottomano_toml_config::Config;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    indicator_groups::{
        journal_events_group::{JournalEventsInteraction, JournalScopeGroup, shown_while_alerting},
        terminal_command::TerminalCommand,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
        value_cells::ValueCell,
    },
};

impl From<&CoredumpEntry> for TableRecord<4> {
    fn from(coredump_entry: &CoredumpEntry) -> Self {
        TableRecord::with_cells(
            ValueCell::Time {
                at: coredump_entry.at.into(),
                format: LocalizedTimeFormat::TimeOfDay,
            },
            [
                coredump_entry.command.clone().into(),
                coredump_entry.signal.to_string().into(),
                coredump_entry.pid.to_string().into(),
                match &coredump_entry.unit {
                    CoredumpOwner::Unit(unit) => unit.name().into(),
                    CoredumpOwner::OutsideUnits => ValueCell::Blank,
                },
            ],
        )
        .level(coredump_entry.severity)
    }
}

pub(super) const COREDUMP_LOG_GLYPH: Phosphor = Phosphor::Bug(Weight::Regular);

impl JournalScopeGroup {
    pub(super) fn wired_coredump_log_indicator<'view>(
        &'view self,
        config: &Config,
        coredump_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        coredump_indicator
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
            }))
    }
}

pub(super) fn coredump_log_state(entries: &[CoredumpEntry]) -> IndicatorState {
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

pub(super) fn coredump_log_indicator(entries: &[CoredumpEntry]) -> GroupIndicator<'_> {
    GroupIndicator::glyph_only(
        "Coredumps",
        COREDUMP_LOG_GLYPH.into(),
        coredump_log_state(entries),
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Unlabeled,
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Counted(entries.len()),
    })
    .with_panel_visibility(coredump_log_panel_visibility(entries))
    .with_hover_card(Box::new(move |_| coredump_log_hover_card(entries)))
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
