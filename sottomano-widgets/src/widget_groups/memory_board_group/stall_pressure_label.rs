use chrono::Locale;
use sottomano_desktop_sources::cpu_memory_compute::memory_stall_pressure::{
    MemoryStallPressure, StallAverages,
};
use sottomano_toml_config::severity::Severity;
use uom::si::ratio::percent;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::GroupItemReading, reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        keyed_record_table::{ColumnHeading, IntoTableRecord, TableRecord},
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

impl IntoTableRecord<3> for StalledTasksRow {
    fn into_table_record(self) -> TableRecord<3> {
        TableRecord::new(
            self.stalled_tasks,
            [
                Quantity::share(self.averages.last_10_seconds).into(),
                Quantity::share(self.averages.last_minute).into(),
                Quantity::share(self.averages.last_5_minutes).into(),
            ],
        )
    }
}

impl GroupItemReading for MemoryStallPressure {
    fn name(&self) -> &'static str {
        "RAM pressure"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Gauge(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(Quantity::share(self.some.last_minute).localized_text(locale))
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        match self.some.last_minute.get::<percent>() {
            stalled if stalled >= STALL_CRITICAL_PERCENT => Severity::Critical,
            stalled if stalled >= STALL_WARNING_PERCENT => Severity::Warning,
            _ => Severity::Normal,
        }
        .into()
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::column_table(
            STALL_WINDOW_HEADINGS,
            [
                StalledTasksRow {
                    stalled_tasks: "Some tasks stalled",
                    averages: self.some,
                },
                StalledTasksRow {
                    stalled_tasks: "All tasks stalled",
                    averages: self.full,
                },
            ],
        )])
    }
}

#[cfg(test)]
mod tests {
    use uom::si::f64::Ratio;

    use super::*;
    use crate::widgets::ongoing_toast::reading_display_state_alert_severity;

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
            stalled_for(7.0).state(Clock::real()),
            ReadingDisplayState::Idle
        );
        assert_eq!(
            stalled_for(25.0).state(Clock::real()),
            ReadingDisplayState::Warning
        );
        assert_eq!(
            stalled_for(50.0).state(Clock::real()),
            ReadingDisplayState::Critical
        );
    }

    #[test]
    fn pressure_never_raises_a_toast() {
        assert_eq!(
            reading_display_state_alert_severity(&stalled_for(80.0), Clock::real()),
            None
        );
    }
}
