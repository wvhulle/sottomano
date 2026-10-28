use chrono::Locale;
use iced::alignment::Horizontal;
use sottomano_sources::systemd_units_journal::journal_previous_shutdown::{
    JobResult, LastShutdown, ShutdownReadError, StopOutcome, UnitStop,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::ValueCell,
        widget_state::WidgetState,
    },
};

impl From<ShutdownReadError> for Section {
    fn from(error: ShutdownReadError) -> Self {
        Section::error(error)
    }
}

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
        TableRecord::new(self.unit.name(), cells).level(self.outcome.severity())
    }
}

impl BarWidget for LastShutdown {
    fn name(&self) -> &'static str {
        "Last shutdown"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Power(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(duration_text(self.duration, locale))
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        self.slowest_stops
            .iter()
            .map(|stop| stop.outcome.severity())
            .max()
            .unwrap_or_default()
            .into()
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([Section::column_table(
            [
                ColumnHeading::titled("Took"),
                ColumnHeading::titled("Result").align_x(Horizontal::Left),
            ],
            self.slowest_stops.iter(),
        )
        .titled((
            "Slowest stops at last shutdown",
            "From the previous boot's journal: shutdown start to its last entry. Amber stops \
             crossed slow_shutdown_stop_secs or never finished.",
        ))])
    }
}
