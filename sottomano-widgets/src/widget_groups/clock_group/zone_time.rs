use std::{cmp::Ordering, convert::Infallible};

use chrono::{DateTime, Locale, Offset, TimeDelta};
use chrono_tz::Tz;
use iced::futures::Stream;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
};

use crate::{
    clock::Clock,
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widget_groups::clock_group::TICK,
    widgets::{
        localized_time::LocalizedTimeFormat,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
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

impl ZoneTime {
    pub(super) fn indicator(&self, clock: Clock, locale: Locale) -> GroupIndicator<'_> {
        GroupIndicator::glyph_only(
            city_name(self.zone()),
            Phosphor::GlobeHemisphereWest(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(
                LocalizedTimeFormat::TimeOfDay.format_in_own_zone(&self.there(clock), locale),
            ),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    pub(super) fn hover_card(self, clock: Clock) -> Vec<Section> {
        let there = self.there(clock);
        vec![Section::new(
            "Time zone",
            dictionary_table([
                LabeledValue::new("Zone", self.zone().name().to_owned()),
                LabeledValue::new("Abbreviation", there.format("%Z").to_string()),
                LabeledValue::new("UTC offset", there.format("%:z").to_string()),
                LabeledValue::new("Day", self.day_relative_to_local(clock).to_owned()),
                LabeledValue::new(
                    "Offset from local",
                    ValueCell::SignedDuration(TimeDelta::seconds(i64::from(
                        self.seconds_ahead_of_local(clock),
                    ))),
                ),
            ]),
        )]
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
