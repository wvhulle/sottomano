use std::{convert::Infallible, time::Duration};

use chrono::{Datelike, Locale, Utc};
use iced::futures::Stream;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

const DATE_TICK: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate;

impl SnapshotSource for LocalDate {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "Local date";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(DATE_TICK).boundary_aligned_deliveries(|| LocalDate)
    }
}

impl LocalDate {
    pub(super) fn reading_row(clock: Clock, locale: Locale) -> ReadingRow<'static> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(
                    LocalizedTimeFormat::Date.format(&clock.local(), locale),
                ),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                Self::NAME,
                Phosphor::CalendarBlank(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(Self::hover_card),
            )
        }
    }

    pub(super) fn hover_card(clock: Clock) -> Vec<Section> {
        let local = clock.local();
        vec![Section::new(
            "Calendar",
            dictionary_table([
                LabeledValue::new(
                    "Weekday",
                    ValueCell::Time {
                        at: local.with_timezone(&Utc),
                        format: LocalizedTimeFormat::Weekday,
                    },
                ),
                LabeledValue::new("Week", local.iso_week().week().to_string()),
                LabeledValue::new("Day of year", local.ordinal().to_string()),
            ]),
        )]
    }
}
