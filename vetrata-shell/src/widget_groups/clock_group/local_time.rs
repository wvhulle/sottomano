use std::{
    convert::Infallible,
    future::{Future, ready},
};

use chrono::{DateTime, Local, Locale};
use iced::futures::Stream;

use crate::{
    data::source::{DeliveryCadence, ReadingSource, SourceDelivery},
    widget_groups::clock_group::{TICK, polled_at_second_boundaries},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, TooltipBody},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct LocalTime(pub DateTime<Local>);

impl ReadingSource for LocalTime {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "local time";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(TICK);

    fn delivery_stream(inputs: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_at_second_boundaries(inputs, Self::poll)
    }
}

impl LocalTime {
    fn poll(_key: ()) -> impl Future<Output = Result<Self, Infallible>> + Send {
        ready(Ok(LocalTime(Local::now())))
    }
}

impl BarWidget for LocalTime {
    fn name(&self) -> &'static str {
        "Local time"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Clock(Weight::Regular).into()
    }

    fn localized_label(&self, locale: Locale) -> Option<String> {
        Some(LocalizedTimeFormat::TimeOfDay.format(&self.0, locale))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("UTC offset", self.0.format("%:z").to_string()),
            KeyValue::new("Unix time", self.0.timestamp().to_string()),
        ])
    }
}
