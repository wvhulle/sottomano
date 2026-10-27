use std::{cmp::Ordering, convert::Infallible};

use chrono::{DateTime, Locale, Offset, TimeDelta};
use chrono_tz::Tz;
use iced::futures::Stream;
use sottomano_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
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
pub struct ZoneTime(pub Tz);

impl SnapshotSource for ZoneTime {
    type Error = Infallible;
    type StreamInputs = Tz;

    const NAME: &'static str = "Zone time";

    fn delivery_stream(zone: Tz) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(TICK).boundary_aligned_deliveries(move || ZoneTime(zone))
    }
}

impl ZoneTime {
    #[must_use]
    pub fn zone(self) -> Tz {
        self.0
    }

    fn there(self, clock: Clock) -> DateTime<Tz> {
        clock.utc().with_timezone(&self.0)
    }

    fn seconds_ahead_of_local(self, clock: Clock) -> i32 {
        self.there(clock).offset().fix().local_minus_utc()
            - clock.local().offset().local_minus_utc()
    }

    fn day_relative_to_local(self, clock: Clock) -> &'static str {
        match self
            .there(clock)
            .date_naive()
            .cmp(&clock.local().date_naive())
        {
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

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        Some(format!(
            "{} {}",
            city_name(self.zone()),
            LocalizedTimeFormat::TimeOfDay.format_in_own_zone(&self.there(clock), locale)
        ))
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        let there = self.there(clock);
        TooltipBody::dictionary([
            KeyValue::new("Zone", self.zone().name().to_owned()),
            KeyValue::new("Abbreviation", there.format("%Z").to_string()),
            KeyValue::new("UTC offset", there.format("%:z").to_string()),
            KeyValue::new("Day", self.day_relative_to_local(clock).to_owned()),
            KeyValue::new(
                "Offset from local",
                Cell::SignedDuration(TimeDelta::seconds(i64::from(
                    self.seconds_ahead_of_local(clock),
                ))),
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
}
