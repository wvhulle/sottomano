use std::time::SystemTime;

use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    system_protocol_clients::journal_follow::unexpired_events,
    systemd_units_journal::journal_oom_kills::{OomKillEntry, OomKiller},
};
use sottomano_toml_config::Config;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        terminal_command::TerminalCommand,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    panel_menus::services_menu::journal_events_group::{
        JournalEventsInteraction, JournalScopeGroup, shown_while_alerting,
    },
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_time::LocalizedTimeFormat,
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
    pub(super) fn wired_oom_kill_log_indicator<'view>(
        &'view self,
        config: &Config,
        oom_kill_indicator: GroupIndicator<'view>,
        now: SystemTime,
    ) -> GroupIndicator<'view> {
        oom_kill_indicator
            .on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                JournalEventsInteraction::OpenLog(TerminalCommand::journal(
                    config,
                    OomKillEntry::follow_log_args(),
                )),
            )
            .with_hover_card(Box::new(move |_| {
                self.oom_kills.sections_described_by(|held_entries| {
                    oom_kill_log_hover_card(&live_oom_kills(held_entries, now))
                })
            }))
    }
}

pub(super) fn live_oom_kills(entries: &[OomKillEntry], now: SystemTime) -> Vec<OomKillEntry> {
    unexpired_events(entries, now).cloned().collect()
}

pub(super) fn oom_kill_log_state(entries: &[OomKillEntry]) -> IndicatorState {
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

pub(super) fn oom_kill_log_indicator<'view>(entries: Vec<OomKillEntry>) -> GroupIndicator<'view> {
    GroupIndicator::glyph_only(
        "OOM kills",
        OOM_KILL_LOG_GLYPH.into(),
        oom_kill_log_state(&entries),
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Unlabeled,
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Counted(entries.len()),
    })
    .with_panel_visibility(oom_kill_log_panel_visibility(&entries))
    .with_hover_card(Box::new(move |_| oom_kill_log_hover_card(&entries)))
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
