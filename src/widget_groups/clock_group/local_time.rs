use std::{
    convert::Infallible,
    future::{Future, ready},
    time::Duration,
};

use chrono::{DateTime, Local, Locale};
use iced::futures::Stream;

use crate::{
    data::source::{Delivery, PolledSource, Source},
    widget_groups::clock_group::{TICK, polled_at_second_boundaries},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, TooltipBody},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct LocalTime(pub DateTime<Local>);

impl Source for LocalTime {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "local time";
}

impl PolledSource for LocalTime {
    const INTERVAL: Duration = TICK;

    fn poll(_key: ()) -> impl Future<Output = Result<Self, Infallible>> + Send {
        ready(Ok(LocalTime(Local::now())))
    }

    fn polls(key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        polled_at_second_boundaries::<Self>(key)
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
        Some(TimePart::TimeOfDay.format(&self.0, locale))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("UTC offset", self.0.format("%:z").to_string()),
            KeyValue::new("Unix time", self.0.timestamp().to_string()),
        ])
    }
}
