use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::journal_previous_shutdown::{
        JobResult, LastShutdown, StopOutcome, UnitStop,
    },
};
use sottomano_toml_config::Config;

use super::{SystemdManagerGroup, SystemdManagerInteraction};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel,
        indicator_group_content::IndicatorListing, terminal_command::TerminalCommand,
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_durations::duration_text,
        value_cells::ValueCell,
    },
};

const LAST_SHUTDOWN_GLYPH: Phosphor = Phosphor::Power(Weight::Regular);

impl From<&UnitStop> for TableRecord<2> {
    fn from(unit_stop: &UnitStop) -> Self {
        let cells = match &unit_stop.outcome {
            StopOutcome::Unfinished => ["did not finish".into(), ValueCell::Blank],
            StopOutcome::Finished {
                duration,
                job_result,
                ..
            } => [
                ValueCell::duration(*duration),
                if *job_result == JobResult::Done {
                    ValueCell::Blank
                } else {
                    job_result.to_string().into()
                },
            ],
        };
        TableRecord::with_cells(unit_stop.unit.name(), cells).level(unit_stop.outcome.severity())
    }
}

pub(super) fn last_shutdown_hover_card(shutdown: &LastShutdown) -> Vec<Section> {
    vec![Section::new(
        SectionTitle::captioned(
            "Slowest stops at last shutdown",
            "From the previous boot's journal: shutdown start to its last entry. Amber stops \
             crossed slow_shutdown_stop_secs or never finished.",
        ),
        headed_table(
            [
                ColumnHeading::titled("Took"),
                ColumnHeading::titled("Result").align_x(Horizontal::Left),
            ],
            shutdown.slowest_stops.iter(),
        ),
    )]
}

pub(super) fn last_shutdown_indicator(
    shutdown: &LastShutdown,
    locale: Locale,
) -> GroupIndicator<'_> {
    let base = GroupIndicator::glyph_only(
        LastShutdown::NAME,
        Phosphor::Power(Weight::Regular).into(),
        shutdown
            .slowest_stops
            .iter()
            .map(|stop| stop.outcome.severity())
            .max()
            .unwrap_or_default()
            .into(),
    )
    .with_hover_card(Box::new(move |_| last_shutdown_hover_card(shutdown)));
    base.with_label(ReadoutLabel::Labeled(duration_text(
        shutdown.duration,
        locale,
    )))
}

impl SystemdManagerGroup {
    pub(super) fn last_shutdown_listing<'view>(
        &'view self,
        config: &'view Config,
    ) -> IndicatorListing<'view> {
        self.last_shutdown.indicator_listing(
            |held_snapshot| vec![self.wired_last_shutdown_indicator(held_snapshot, config)],
            LAST_SHUTDOWN_GLYPH.into(),
        )
    }

    fn wired_last_shutdown_indicator<'view>(
        &'view self,
        last_shutdown: &'view LastShutdown,
        config: &'view Config,
    ) -> GroupIndicator<'view> {
        last_shutdown_indicator(last_shutdown, config.calendar.locale)
            .on_press(
                TerminalCommand::JOURNAL_PRESS_HINT,
                SystemdManagerInteraction::OpenLog(TerminalCommand::journal(
                    config,
                    last_shutdown.follow_log_args(),
                )),
            )
            .with_hover_card(Box::new(move |_| {
                self.last_shutdown
                    .sections_described_by(last_shutdown_hover_card)
            }))
    }
}
