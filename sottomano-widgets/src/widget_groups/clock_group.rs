use chrono::Locale;

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
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::WidgetGroupFamily,
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
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
    Expansion(WidgetGroupExpansionMessage),
    Refresh(ClockSource),
}

#[derive(Default)]
pub struct ClockGroup {
    time: SubscribedSnapshot<LocalTime>,
    date: SubscribedSnapshot<LocalDate>,
    zone_times: Vec<(Tz, SubscribedSnapshot<ZoneTime>)>,
    expansion: WidgetGroupExpansion,
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
        let demand = demand.expanded_when(self.expansion.state());
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

    pub fn group_content(&self, clock: Clock, locale: Locale) -> WidgetGroupContent<'_> {
        let headline_indicator = match &self.time {
            SubscribedSnapshot::SnapshotHeld { .. } => self
                .local_time_indicator(LocalTime::indicator(clock, locale))
                .with_headline_glyph(Phosphor::Watch(Weight::Regular).into()),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => self
                .local_time_indicator(
                    troubled_snapshot.shown_over(LocalTime::indicator(clock, locale), clock),
                )
                .with_headline_glyph(Phosphor::Watch(Weight::Regular).into()),
            SubscribedSnapshot::NoSnapshot { placeholder } => self
                .local_time_indicator(placeholder.indicator(LOCAL_TIME_GLYPH.into()))
                .with_headline_glyph(Phosphor::Watch(Weight::Regular).into()),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupIndicator::glyph_only(
                WidgetGroupFamily::Clock.title(),
                Phosphor::Watch(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupIndicator::glyph_only(
                WidgetGroupFamily::Clock.title(),
                Phosphor::Watch(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        };
        let mut lead_indicators = Vec::new();
        match &self.time {
            SubscribedSnapshot::SnapshotHeld { .. } => {
                lead_indicators
                    .push(self.local_time_indicator(LocalTime::indicator(clock, locale)));
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                lead_indicators.push(self.local_time_indicator(
                    troubled_snapshot.shown_over(LocalTime::indicator(clock, locale), clock),
                ));
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => lead_indicators
                .push(self.local_time_indicator(placeholder.indicator(LOCAL_TIME_GLYPH.into()))),
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        let mut rows = Vec::new();
        match &self.date {
            SubscribedSnapshot::SnapshotHeld { .. } => {
                let date_indicator = LocalDate::indicator(clock, locale);
                if date_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.local_date_indicator(date_indicator));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let date_indicator =
                    troubled_snapshot.shown_over(LocalDate::indicator(clock, locale), clock);
                if date_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(self.local_date_indicator(date_indicator));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(
                    self.local_date_indicator(placeholder.indicator(LOCAL_DATE_GLYPH.into())),
                );
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        for (zone, zone_time) in &self.zone_times {
            match zone_time {
                SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                    let zone_indicator = held_snapshot.indicator(clock, locale);
                    if zone_indicator.panel_visibility() == PanelVisibility::Shown {
                        rows.push(Self::zone_time_indicator(*zone, zone_time, zone_indicator));
                    }
                }
                SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                    let zone_indicator = troubled_snapshot.shown_over(
                        troubled_snapshot.held_snapshot.indicator(clock, locale),
                        clock,
                    );
                    if zone_indicator.panel_visibility() == PanelVisibility::Shown {
                        rows.push(Self::zone_time_indicator(*zone, zone_time, zone_indicator));
                    }
                }
                SubscribedSnapshot::NoSnapshot { placeholder } => {
                    rows.push(Self::zone_time_indicator(
                        *zone,
                        zone_time,
                        placeholder.indicator(ZONE_TIME_GLYPH.into()),
                    ));
                }
                SubscribedSnapshot::AwaitingFirstSnapshot
                | SubscribedSnapshot::SourceUnavailable => {}
            }
        }
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Clock,
            headline_indicator,
            &self.expansion,
            ClockInteraction::Expansion,
            lead_indicators.into_iter().chain(rows),
        )
    }

    fn local_time_indicator<'a>(
        &'a self,
        time_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        time_indicator
            .with_hover_card(Box::new(|card_clock| {
                self.time
                    .sections_described_by(|_| LocalTime::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalTime))
    }

    fn local_date_indicator<'a>(
        &'a self,
        date_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        date_indicator
            .with_hover_card(Box::new(|card_clock| {
                self.date
                    .sections_described_by(|_| LocalDate::hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalDate))
    }

    fn zone_time_indicator<'a>(
        zone: Tz,
        zone_time: &'a SubscribedSnapshot<ZoneTime>,
        zone_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        zone_indicator
            .with_hover_card(Box::new(|card_clock| {
                zone_time.sections_described_by(|snapshot| snapshot.hover_card(card_clock))
            }))
            .refreshed_by(ClockInteraction::Refresh(ClockSource::ZoneTime(zone)))
    }
}
