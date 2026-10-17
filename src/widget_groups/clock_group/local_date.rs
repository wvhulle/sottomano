use std::{
    convert::Infallible,
    future::{Future, ready},
};

use chrono::{DateTime, Datelike, Local, Locale, Utc};
use iced::futures::Stream;

use crate::{
    data::source::{DeliveryCadence, ReadingSource, SourceDelivery},
    widget_groups::clock_group::{TICK, polled_at_second_boundaries},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, TooltipBody},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct LocalDate(pub DateTime<Local>);

impl ReadingSource for LocalDate {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "local date";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(TICK);

    fn delivery_stream(key: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_at_second_boundaries(key, Self::poll)
    }
}

impl LocalDate {
    fn poll(_key: ()) -> impl Future<Output = Result<Self, Infallible>> + Send {
        ready(Ok(LocalDate(Local::now())))
    }
}

impl BarWidget for LocalDate {
    fn name(&self) -> &'static str {
        "Local date"
    }

    fn icon(&self) -> Glyph {
        Phosphor::CalendarBlank(Weight::Regular).into()
    }

    fn localized_label(&self, locale: Locale) -> Option<String> {
        Some(LocalizedTimeFormat::Date.format(&self.0, locale))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new(
                "Weekday",
                Cell::Time {
                    at: self.0.with_timezone(&Utc),
                    format: LocalizedTimeFormat::Weekday,
                },
            ),
            KeyValue::new("Week", self.0.iso_week().week().to_string()),
            KeyValue::new("Day of year", self.0.ordinal().to_string()),
        ])
    }
}
