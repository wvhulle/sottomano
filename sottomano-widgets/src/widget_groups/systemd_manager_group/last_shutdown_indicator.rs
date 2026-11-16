use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::journal_previous_shutdown::{
        JobResult, LastShutdown, StopOutcome, UnitStop,
    },
};

use crate::{
    group_indicators::{group_indicator::GroupIndicator, indicator_drawing::ReadoutLabel},
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

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
