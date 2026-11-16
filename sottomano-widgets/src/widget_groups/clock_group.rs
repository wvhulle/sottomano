use chrono::Locale;

use crate::widgets::icon_glyph::IconGlyph;
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
    group_items::group_item::GroupItem,
    group_menus::group_source_demand::GroupSourceDemand,
    widget_groups::{
        group_summary::GroupSummary,
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
            ClockMessage::Time(reading) => self.time.receive(reading, clock),
            ClockMessage::Date(reading) => self.date.receive(reading, clock),
            ClockMessage::Zone(zone, reading) => {
                self.zone_time_mut(zone).receive(reading, clock);
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
            demand.while_rows_shown(
                snapshot_subscription::<ZoneTime>(*zone)
                    .with(*zone)
                    .map(|(zone, reading)| ClockMessage::Zone(zone, reading)),
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
            ClockSource::ZoneTime(zone) => first_delivery::<ZoneTime>(zone)
                .map(move |reading| ClockMessage::Zone(zone, reading)),
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

    pub fn group_content(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, ClockInteraction> {
        let summarised_row = GroupItem::reading(self.time.main_row(
            Phosphor::Clock(Weight::Regular).into(),
            |_| LocalTime::reading_row(clock, locale),
            clock,
        ))
        .with_hover_card(Box::new(|card_clock| {
            self.time
                .sections_described_by(|_| LocalTime::hover_card(card_clock))
        }))
        .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalTime));
        let group_summary =
            GroupSummary::showing(Phosphor::Clock(Weight::Regular).into(), &summarised_row);
        WidgetGroupContent::expandable(
            WidgetGroupFamily::Clock,
            group_summary,
            &self.expansion,
            ClockInteraction::Expansion,
            [
                Some(summarised_row),
                self.date
                    .group_item(
                        IconGlyph::default(),
                        |_| LocalDate::reading_row(clock, locale),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(|card_clock| {
                            self.date
                                .sections_described_by(|_| LocalDate::hover_card(card_clock))
                        }))
                        .refreshed_by(ClockInteraction::Refresh(ClockSource::LocalDate))
                    }),
            ]
            .into_iter()
            .chain(self.zone_times.iter().map(|(zone, zone_time)| {
                zone_time
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| snapshot.reading_row(clock, locale),
                        clock,
                    )
                    .map(|item| {
                        item.with_hover_card(Box::new(|card_clock| {
                            zone_time
                                .sections_described_by(|snapshot| snapshot.hover_card(card_clock))
                        }))
                        .refreshed_by(ClockInteraction::Refresh(ClockSource::ZoneTime(*zone)))
                    })
            }))
            .flatten(),
        )
    }
}
