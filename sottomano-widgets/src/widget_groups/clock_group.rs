mod local_date;
mod local_time;
mod zone_time;

use std::time::Duration;

use chrono_tz::Tz;
use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::SnapshotDelivery;

use self::{local_date::LocalDate, local_time::LocalTime, zone_time::ZoneTime};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        latest_snapshot::LatestSnapshot,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

pub const TICK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq)]
pub enum ClockTarget {
    LocalTime,
    LocalDate,
    ZoneTime(Tz),
}

impl From<ClockTarget> for HoverTarget {
    fn from(target: ClockTarget) -> Self {
        HoverTarget::Clock(target)
    }
}

#[derive(Default)]
pub struct ClockGroup {
    time: LatestSnapshot<LocalTime>,
    date: LatestSnapshot<LocalDate>,
    zone_times: Vec<(Tz, LatestSnapshot<ZoneTime>)>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ClockMessage {
    Time(SnapshotDelivery<LocalTime>),
    Date(SnapshotDelivery<LocalDate>),
    Zone(Tz, SnapshotDelivery<ZoneTime>),
    Expansion(ExpansionMessage),
}

impl ClockGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, msg: ClockMessage, clock: Clock) -> Task<ClockMessage> {
        match msg {
            ClockMessage::Time(reading) => self.time.receive(reading, clock),
            ClockMessage::Date(reading) => self.date.receive(reading, clock),
            ClockMessage::Zone(zone, reading) => {
                self.zone_time_mut(zone).receive(reading, clock);
            }
            ClockMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn subscription(config: &Config) -> Subscription<ClockMessage> {
        let zones = config.calendar.other_time_zones.iter().map(|zone| {
            snapshot_subscription::<ZoneTime>(*zone)
                .with(*zone)
                .map(|(zone, reading)| ClockMessage::Zone(zone, reading))
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

    pub fn refresh(target: &ClockTarget) -> Task<ClockMessage> {
        match *target {
            ClockTarget::LocalTime => first_delivery::<LocalTime>(()).map(ClockMessage::Time),
            ClockTarget::LocalDate => first_delivery::<LocalDate>(()).map(ClockMessage::Date),
            ClockTarget::ZoneTime(zone) => first_delivery::<ZoneTime>(zone)
                .map(move |reading| ClockMessage::Zone(zone, reading)),
        }
    }

    fn zone_time_mut(&mut self, zone: Tz) -> &mut LatestSnapshot<ZoneTime> {
        let position = self
            .zone_times
            .iter()
            .position(|(known, _)| *known == zone)
            .unwrap_or_else(|| {
                self.zone_times.push((zone, LatestSnapshot::default()));
                self.zone_times.len() - 1
            });
        &mut self.zone_times[position].1
    }

    fn zone_time(&self, zone: Tz) -> Option<&LatestSnapshot<ZoneTime>> {
        self.zone_times
            .iter()
            .find(|(known, _)| *known == zone)
            .map(|(_, zone_time)| zone_time)
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &ClockTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            ClockTarget::LocalTime => self.time.tooltip(clock),
            ClockTarget::LocalDate => self.date.tooltip(clock),
            ClockTarget::ZoneTime(zone) => self.zone_time(*zone)?.tooltip(clock),
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, ClockMessage> {
        GroupContent::new(
            &self.expansion,
            ClockMessage::Expansion,
            [
                self.time.group_item(ClockTarget::LocalTime),
                self.date.group_item(ClockTarget::LocalDate),
            ]
            .into_iter()
            .chain(
                self.zone_times
                    .iter()
                    .map(|(zone, zone_time)| zone_time.group_item(ClockTarget::ZoneTime(*zone))),
            )
            .flatten(),
        )
    }
}
