use chrono::Locale;

use crate::indicator_groups::indicator_group_content::IndicatorListing;

mod local_date;
mod local_time;
mod zone_time;

use std::time::Duration;

use chrono_tz::Tz;
use iced::{Subscription, Task};
use sottomano_desktop_sources::SnapshotDelivery;
use sottomano_toml_config::Config;

use self::{local_date::LocalDate, local_time::LocalTime, zone_time::ZoneTime};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::{GroupIndicator, PanelVisibility},
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::IndicatorGroupFamily,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

pub const TICK: Duration = Duration::from_secs(1);

const LOCAL_TIME_GLYPH: Phosphor = Phosphor::Clock(Weight::Regular);
const LOCAL_DATE_GLYPH: Phosphor = Phosphor::CalendarBlank(Weight::Regular);
const ZONE_TIME_GLYPH: Phosphor = Phosphor::GlobeHemisphereWest(Weight::Regular);

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
            .local_time_indicator(self.time.indicator(
                |_| LocalTime::indicator(clock, locale),
                LOCAL_TIME_GLYPH.into(),
            ))
            .with_headline_glyph(Phosphor::Watch(Weight::Regular).into());
        let mut lead_indicators = Vec::new();
        if !self.time.awaits_first_delivery() {
            lead_indicators.push(self.local_time_indicator(self.time.indicator(
                |_| LocalTime::indicator(clock, locale),
                LOCAL_TIME_GLYPH.into(),
            )));
        }
        let mut rows = Vec::new();
        if !self.date.awaits_first_delivery() {
            let date_indicator = self.date.indicator(
                |_| LocalDate::indicator(clock, locale),
                LOCAL_DATE_GLYPH.into(),
            );
            if date_indicator.panel_visibility == PanelVisibility::Shown {
                rows.push(self.local_date_indicator(date_indicator));
            }
        }
        for (zone, zone_time) in &self.zone_times {
            if !zone_time.awaits_first_delivery() {
                let zone_indicator = zone_time.indicator(
                    |held_snapshot| held_snapshot.indicator(clock, locale),
                    ZONE_TIME_GLYPH.into(),
                );
                if zone_indicator.panel_visibility == PanelVisibility::Shown {
                    rows.push(Self::zone_time_indicator(*zone, zone_time, zone_indicator));
                }
            }
        }
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::Clock,
            headline_indicator,
            &self.expansion,
            ClockInteraction::Expansion,
            IndicatorListing::Listed(lead_indicators.into_iter().chain(rows).collect()),
        )
    }

    fn local_time_indicator<'view>(
        &'view self,
        time_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        time_indicator
            .with_hover_card(Box::new(|card_clock| {
                self.time
                    .sections_described_by(|_| LocalTime::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalTime))
    }

    fn local_date_indicator<'view>(
        &'view self,
        date_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        date_indicator
            .with_hover_card(Box::new(|card_clock| {
                self.date
                    .sections_described_by(|_| LocalDate::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalDate))
    }

    fn zone_time_indicator<'view>(
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
