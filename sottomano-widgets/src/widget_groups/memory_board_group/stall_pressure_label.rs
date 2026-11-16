use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::memory_stall_pressure::{MemoryStallPressure, StallAverages},
};
use sottomano_toml_config::severity::Severity;
use uom::si::ratio::percent;

use crate::{
    group_indicators::{
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, headed_table},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
    },
};

const STALL_WARNING_PERCENT: f64 = 20.0;
const STALL_CRITICAL_PERCENT: f64 = 40.0;

const STALL_WINDOW_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::titled("10 s"),
    ColumnHeading::titled("1 min"),
    ColumnHeading::titled("5 min"),
];

struct StalledTasksRow {
    stalled_tasks: &'static str,
    averages: StallAverages,
}

impl From<StalledTasksRow> for TableRecord<3> {
    fn from(stalled_tasks_row: StalledTasksRow) -> Self {
        TableRecord::with_cells(
            stalled_tasks_row.stalled_tasks,
            [
                Quantity::share(stalled_tasks_row.averages.last_10_seconds).into(),
                Quantity::share(stalled_tasks_row.averages.last_minute).into(),
                Quantity::share(stalled_tasks_row.averages.last_5_minutes).into(),
            ],
        )
    }
}

pub fn memory_stall_indicator(pressure: &MemoryStallPressure, locale: Locale) -> ReadingRow<'_> {
    let state = match pressure.some.last_minute.get::<percent>() {
        stalled if stalled >= STALL_CRITICAL_PERCENT => Severity::Critical,
        stalled if stalled >= STALL_WARNING_PERCENT => Severity::Warning,
        _ => Severity::Normal,
    }
    .into();
    ReadingRow {
        readout: IndicatorReadout {
            label: ReadoutLabel::Labeled(
                Quantity::share(pressure.some.last_minute).localized_text(locale),
            ),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            MemoryStallPressure::NAME,
            Phosphor::Gauge(Weight::Regular).into(),
            state,
            Box::new(move |_| memory_stall_hover_card(pressure)),
        )
    }
}

pub fn memory_stall_hover_card(pressure: &MemoryStallPressure) -> Vec<Section> {
    vec![Section::new(
        "Averages",
        headed_table(
            STALL_WINDOW_HEADINGS,
            [
                StalledTasksRow {
                    stalled_tasks: "Some tasks stalled",
                    averages: pressure.some,
                },
                StalledTasksRow {
                    stalled_tasks: "All tasks stalled",
                    averages: pressure.full,
                },
            ],
        ),
    )]
}

#[cfg(test)]
mod tests {
    use uom::si::f64::Ratio;

    use super::*;
    use crate::group_indicators::{
        group_indicator::indicator_toast_severity, indicator_state::IndicatorState,
    };

    fn stalled_for(last_minute_percent: f64) -> MemoryStallPressure {
        let averages = StallAverages {
            last_10_seconds: Ratio::new::<percent>(last_minute_percent),
            last_minute: Ratio::new::<percent>(last_minute_percent),
            last_5_minutes: Ratio::new::<percent>(last_minute_percent),
        };
        MemoryStallPressure {
            some: averages,
            full: averages,
        }
    }

    #[test]
    fn sustained_stalls_set_the_state() {
        assert_eq!(
            memory_stall_indicator(&stalled_for(7.0), Locale::default()).shown_state(),
            IndicatorState::Idle
        );
        assert_eq!(
            memory_stall_indicator(&stalled_for(25.0), Locale::default()).shown_state(),
            IndicatorState::Warning
        );
        assert_eq!(
            memory_stall_indicator(&stalled_for(50.0), Locale::default()).shown_state(),
            IndicatorState::Critical
        );
    }

    #[test]
    fn pressure_never_raises_a_toast() {
        assert_eq!(
            indicator_toast_severity(&memory_stall_indicator(
                &stalled_for(80.0),
                Locale::default()
            )),
            None
        );
    }
}
