use std::{cmp::Ordering, convert::Infallible};

use chrono::{DateTime, Locale, Offset, TimeDelta};
use chrono_tz::Tz;
use iced::futures::Stream;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
};

use super::{ClockGroup, ClockInteraction, ClockSource};
use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    panel_menus::agenda_menu::clock_group::TICK,
    widgets::{
        localized_time::LocalizedTimeFormat,
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::{LabeledValue, ValueCell},
    },
};

const ZONE_TIME_GLYPH: Phosphor = Phosphor::GlobeHemisphereWest(Weight::Regular);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneTime {
    pub time_zone: Tz,
}

impl SnapshotSource for ZoneTime {
    type Error = Infallible;
    type StreamInputs = Tz;

    const NAME: &'static str = "Zone time";

    fn delivery_stream(zone: Tz) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        PeriodicPolling::every(TICK)
            .boundary_aligned_deliveries(move || ZoneTime { time_zone: zone })
    }
}

impl ZoneTime {
    fn there(self, clock: Clock) -> DateTime<Tz> {
        clock.utc().with_timezone(&self.time_zone)
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
            city_name(self.time_zone),
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
                LabeledValue::new("Zone", self.time_zone.name().to_owned()),
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

impl ClockGroup {
    pub(super) fn zone_time_listings(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> impl Iterator<Item = IndicatorListing<'_>> {
        self.zone_times.iter().map(move |(zone, zone_time)| {
            zone_time
                .indicator_listing(
                    |held_snapshot| held_snapshot.indicator(clock, locale).when_shown(),
                    ZONE_TIME_GLYPH.into(),
                )
                .map_indicators(|zone_row| {
                    Self::wired_zone_time_indicator(*zone, zone_time, zone_row)
                })
        })
    }

    fn wired_zone_time_indicator<'view>(
        zone: Tz,
        zone_time: &'view SubscribedSnapshot<ZoneTime>,
        zone_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        zone_indicator
            .with_hover_card(Box::new(|card_clock| {
                zone_time.sections_described_by(|snapshot| snapshot.hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::ZoneTime(zone)))
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
}
