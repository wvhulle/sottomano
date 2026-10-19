use std::convert::Infallible;

use chrono::Locale;
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
        tooltip_body::{KeyValue, TooltipBody},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime;

impl ReadingSource for LocalTime {
    type Error = Infallible;
    type StreamInputs = ();

    const NAME: &'static str = "local time";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(TICK);

    fn delivery_stream((): ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        ticked_at_boundaries(TICK, || LocalTime)
    }
}

impl BarWidget for LocalTime {
    fn name(&self) -> &'static str {
        "Local time"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Clock(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        Some(LocalizedTimeFormat::TimeOfDay.format(&clock.local(), locale))
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let local = clock.local();
        TooltipBody::dictionary([
            KeyValue::new("UTC offset", local.format("%:z").to_string()),
            KeyValue::new("Unix time", local.timestamp().to_string()),
        ])
    }
}
