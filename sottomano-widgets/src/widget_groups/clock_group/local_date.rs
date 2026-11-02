use std::convert::Infallible;

use chrono::{Datelike, Locale, Utc};
use iced::futures::Stream;
use sottomano_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
};

use crate::{
    clock::Clock,
    widget_groups::clock_group::TICK,
    widgets::{
        group_item_reading::GroupItemReading,
        hover_card_body::HoverCardBody,
        icon_glyph::IconGlyph,
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate;

impl SnapshotSource for LocalDate {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "Local date";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(TICK).boundary_aligned_deliveries(|| LocalDate)
    }
}

impl GroupItemReading for LocalDate {
    fn name(&self) -> &'static str {
        "Local date"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::CalendarBlank(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        Some(LocalizedTimeFormat::Date.format(&clock.local(), locale))
    }

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        let local = clock.local();
        HoverCardBody::dictionary([
            LabeledValue::new(
                "Weekday",
                ValueCell::Time {
                    at: local.with_timezone(&Utc),
                    format: LocalizedTimeFormat::Weekday,
                },
            ),
            LabeledValue::new("Week", local.iso_week().week().to_string()),
            LabeledValue::new("Day of year", local.ordinal().to_string()),
        ])
    }
}
