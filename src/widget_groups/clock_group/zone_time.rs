use std::{
    cmp::Ordering,
    convert::Infallible,
    future::{Future, ready},
};

use chrono::{DateTime, Local, Locale, Offset};
use chrono_tz::Tz;
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
pub struct ZoneTime(pub DateTime<Tz>);

impl ReadingSource for ZoneTime {
    type Error = Infallible;
    type Key = Tz;

    const NAME: &'static str = "zone time";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(TICK);

    fn delivery_stream(zone: Tz) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        polled_at_second_boundaries(zone, Self::poll)
    }
}

impl ZoneTime {
    fn poll(zone: Tz) -> impl Future<Output = Result<Self, Infallible>> + Send {
        ready(Ok(ZoneTime(Local::now().with_timezone(&zone))))
    }
}

impl ZoneTime {
    pub fn zone(&self) -> Tz {
        self.0.timezone()
    }

    fn local(&self) -> DateTime<Local> {
        self.0.with_timezone(&Local)
    }

    fn seconds_ahead_of_local(&self) -> i32 {
        self.0.offset().fix().local_minus_utc() - self.local().offset().local_minus_utc()
    }

    fn day_relative_to_local(&self) -> &'static str {
        match self.0.date_naive().cmp(&self.local().date_naive()) {
            Ordering::Less => "Previous day",
            Ordering::Equal => "Same day",
            Ordering::Greater => "Next day",
        }
    }
}

impl BarWidget for ZoneTime {
    fn name(&self) -> &'static str {
        "Zone time"
    }

    fn icon(&self) -> Glyph {
        Phosphor::GlobeHemisphereWest(Weight::Regular).into()
    }

    fn localized_label(&self, locale: Locale) -> Option<String> {
        Some(format!(
            "{} {}",
            city_name(self.zone()),
            LocalizedTimeFormat::TimeOfDay.format_in_own_zone(&self.0, locale)
        ))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("Zone", self.zone().name().to_owned()),
            KeyValue::new("Abbreviation", self.0.format("%Z").to_string()),
            KeyValue::new("UTC offset", self.0.format("%:z").to_string()),
            KeyValue::new("Day", self.day_relative_to_local().to_owned()),
            KeyValue::new(
                "Offset from local",
                signed_hours_and_minutes(self.seconds_ahead_of_local()),
            ),
        ])
    }
}

fn city_name(zone: Tz) -> String {
    zone.name()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .replace('_', " ")
}

fn signed_hours_and_minutes(seconds: i32) -> String {
    let sign = if seconds < 0 { '−' } else { '+' };
    let minutes = seconds.unsigned_abs() / 60;
    let hours = minutes / 60;
    match minutes % 60 {
        0 => format!("{sign}{hours} h"),
        remaining => format!("{sign}{hours} h {remaining} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_name_is_the_last_zone_segment_with_spaces() {
        assert_eq!(
            city_name(Tz::America__Argentina__Buenos_Aires),
            "Buenos Aires"
        );
        assert_eq!(city_name(Tz::UTC), "UTC");
    }

    #[test]
    fn offsets_show_sign_hours_and_leftover_minutes() {
        assert_eq!(signed_hours_and_minutes(-6 * 3600), "−6 h");
        assert_eq!(signed_hours_and_minutes(3 * 3600 + 1800), "+3 h 30 min");
        assert_eq!(signed_hours_and_minutes(0), "+0 h");
    }
}
