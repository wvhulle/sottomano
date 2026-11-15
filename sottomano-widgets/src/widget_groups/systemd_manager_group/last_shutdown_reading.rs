use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_desktop_sources::{
    SnapshotSource,
    systemd_units_journal::journal_previous_shutdown::{
        JobResult, LastShutdown, StopOutcome, UnitStop,
    },
};

use crate::{
    clock::Clock,
    group_items::{
        reading_row::ReadingRow,
        source_reading::{ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        value_cells::ValueCell,
    },
};

impl IntoTableRecord<2> for &UnitStop {
    fn into_table_record(self) -> TableRecord<2> {
        let cells = match &self.outcome {
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
        TableRecord::with_cells(self.unit.name(), cells).level(self.outcome.severity())
    }
}

pub(super) fn last_shutdown_hover_card(shutdown: &LastShutdown, _clock: Clock) -> Vec<Section> {
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

pub(super) fn last_shutdown_reading_row(
    shutdown: &LastShutdown,
    _clock: Clock,
    locale: Locale,
) -> ReadingRow<'_> {
    let base = ReadingRow::glyph_only(
        LastShutdown::NAME,
        Phosphor::Power(Weight::Regular).into(),
        shutdown
            .slowest_stops
            .iter()
            .map(|stop| stop.outcome.severity())
            .max()
            .unwrap_or_default()
            .into(),
        Box::new(move |card_clock| last_shutdown_hover_card(shutdown, card_clock)),
    );
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(duration_text(shutdown.duration, locale)),
            ..base.readout
        },
        ..base
    }
}
