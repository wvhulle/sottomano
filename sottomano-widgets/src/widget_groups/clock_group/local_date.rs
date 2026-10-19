use std::convert::Infallible;

use chrono::{Datelike, Locale, Utc};
use iced::futures::Stream;
use sottomano_sources::{
    DeliveryCadence, ReadingSource, SourceDelivery, stream_operators::ticked_at_boundaries,
};

use crate::{
    clock::Clock,
    widget_groups::clock_group::TICK,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, TooltipBody},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate;

impl ReadingSource for LocalDate {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "local date";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(TICK);

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        ticked_at_boundaries(TICK, || LocalDate)
    }
}

impl BarWidget for LocalDate {
    fn name(&self) -> &'static str {
        "Local date"
    }

    fn icon(&self) -> Glyph {
        Phosphor::CalendarBlank(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        Some(LocalizedTimeFormat::Date.format(&clock.local(), locale))
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let local = clock.local();
        TooltipBody::dictionary([
            KeyValue::new(
                "Weekday",
                Cell::Time {
                    at: local.with_timezone(&Utc),
                    format: LocalizedTimeFormat::Weekday,
                },
            ),
            KeyValue::new("Week", local.iso_week().week().to_string()),
            KeyValue::new("Day of year", local.ordinal().to_string()),
        ])
    }
}
