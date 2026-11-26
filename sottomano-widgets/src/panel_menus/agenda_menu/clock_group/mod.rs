use chrono::Locale;

mod local_date_indicator;
mod local_time_indicator;
mod zone_time_indicator;

use std::time::Duration;

use chrono_tz::Tz;
use iced::{Subscription, Task};
use sottomano_desktop_sources::SnapshotDelivery;
use sottomano_toml_config::Config;

use self::{
    local_date_indicator::LocalDate,
    local_time_indicator::{LOCAL_TIME_GLYPH, LocalTime},
    zone_time_indicator::ZoneTime,
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub const TICK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockSource {
    LocalTime,
    LocalDate,
    ZoneTime(Tz),
}

#[derive(Debug, Clone, Copy)]
pub enum ClockInteraction {
    Expansion(IndicatorGroupExpansionMessage),
    Refresh(ClockSource),
}

#[derive(Default)]
pub struct ClockGroup {
    time: SubscribedSnapshot<LocalTime>,
    date: SubscribedSnapshot<LocalDate>,
    zone_times: Vec<(Tz, SubscribedSnapshot<ZoneTime>)>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum ClockMessage {
    Time(SnapshotDelivery<LocalTime>),
    Date(SnapshotDelivery<LocalDate>),
    Zone(Tz, SnapshotDelivery<ZoneTime>),
}

impl ClockGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, message: ClockMessage, clock: Clock) -> Task<ClockMessage> {
        match message {
            ClockMessage::Time(sample) => self.time.receive(sample, clock),
            ClockMessage::Date(sample) => self.date.receive(sample, clock),
            ClockMessage::Zone(zone, sample) => {
                self.zone_time_mut(zone).receive(sample, clock);
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: ClockInteraction) -> Task<ClockMessage> {
        match interaction {
            ClockInteraction::Expansion(expansion_message) => {
                self.expansion.update(expansion_message);
                Task::none()
            }
            ClockInteraction::Refresh(source) => Self::refresh(source),
        }
    }

    pub fn subscription(
        &self,
        config: &Config,
        demand: GroupSourceDemand,
    ) -> Subscription<ClockMessage> {
        let demand = demand.expanded_when(self.expansion.state);
        let zones = config.calendar.other_time_zones.iter().map(|zone| {
            demand.while_indicators_shown(
                snapshot_subscription::<ZoneTime>(*zone)
                    .with(*zone)
                    .map(|(zone, sample)| ClockMessage::Zone(zone, sample)),
            )
        });
        Subscription::batch(
            [
                snapshot_subscription::<LocalTime>(()).map(ClockMessage::Time),
                snapshot_subscription::<LocalDate>(()).map(ClockMessage::Date),
            ]
            .into_iter()
            .chain(zones),
        )
    }

    fn refresh(source: ClockSource) -> Task<ClockMessage> {
        match source {
            ClockSource::LocalTime => first_delivery::<LocalTime>(()).map(ClockMessage::Time),
            ClockSource::LocalDate => first_delivery::<LocalDate>(()).map(ClockMessage::Date),
            ClockSource::ZoneTime(zone) => {
                first_delivery::<ZoneTime>(zone).map(move |sample| ClockMessage::Zone(zone, sample))
            }
        }
    }

    fn zone_time_mut(&mut self, zone: Tz) -> &mut SubscribedSnapshot<ZoneTime> {
        let position = self
            .zone_times
            .iter()
            .position(|(known, _)| *known == zone)
            .unwrap_or_else(|| {
                self.zone_times.push((zone, SubscribedSnapshot::default()));
                self.zone_times.len() - 1
            });
        &mut self.zone_times[position].1
    }

    pub fn group_content(&self, clock: Clock, locale: Locale) -> IndicatorGroupContent<'_> {
        let headline_indicator = self
            .wired_local_time_indicator(self.time.indicator(
                |_| LocalTime::indicator(clock, locale),
                LOCAL_TIME_GLYPH.into(),
            ))
            .with_headline_glyph(Phosphor::Watch(Weight::Regular).into());
        let indicator_listing = [
            self.local_time_listing(clock, locale),
            self.local_date_listing(clock, locale),
        ]
        .into_iter()
        .chain(self.zone_time_listings(clock, locale))
        .collect();
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Clock,
            headline_indicator,
            &self.expansion,
            ClockInteraction::Expansion,
            indicator_listing,
        )
    }
}
