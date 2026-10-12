mod local_date;
mod local_time;
mod zone_time;

use std::{convert::identity, time::Duration};

use chrono::{DateTime, Local};
use chrono_tz::Tz;
use iced::{
    Subscription, Task,
    futures::{Stream, StreamExt, stream},
};
use tokio::time::sleep;

use self::{local_date::LocalDate, local_time::LocalTime, zone_time::ZoneTime};
use crate::{
    config::Config,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        continuous_reading::ContinuousReading,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

const TICK: Duration = Duration::from_secs(1);

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

pub struct ClockGroup {
    time: ContinuousReading<LocalTime>,
    date: ContinuousReading<LocalDate>,
    zone_times: Vec<ContinuousReading<ZoneTime>>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone, Copy)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
    Expansion(ExpansionMessage),
}

impl Default for ClockGroup {
    fn default() -> Self {
        let mut group = Self {
            time: ContinuousReading::default(),
            date: ContinuousReading::default(),
            zone_times: Vec::new(),
            expansion: GroupExpansion::default(),
        };
        group.tick(Local::now(), &[]);
        group
    }
}

impl ClockGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, msg: ClockMessage, config: &Config) -> Task<ClockMessage> {
        match msg {
            ClockMessage::Tick(now) => self.tick(now, &config.calendar.other_time_zones),
            ClockMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn subscription() -> Subscription<ClockMessage> {
        Subscription::run(ticks)
    }

    fn tick(&mut self, now: DateTime<Local>, other_time_zones: &[Tz]) {
        self.time.load(LocalTime(now));
        self.date.load(LocalDate(now));
        self.zone_times = other_time_zones
            .iter()
            .map(|zone| {
                let mut zone_time = ContinuousReading::default();
                zone_time.load(ZoneTime(now.with_timezone(zone)));
                zone_time
            })
            .collect();
    }

    fn zone_time(&self, zone: Tz) -> Option<&ContinuousReading<ZoneTime>> {
        self.zone_times
            .iter()
            .find(|zone_time| zone_time.get().is_some_and(|time| time.zone() == zone))
    }

    pub fn tooltip_for(&self, target: &ClockTarget) -> Option<Tooltip> {
        match target {
            ClockTarget::LocalTime => self.time.tooltip(),
            ClockTarget::LocalDate => self.date.tooltip(),
            ClockTarget::ZoneTime(zone) => self.zone_time(*zone)?.tooltip(),
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
            .chain(self.zone_times.iter().map(|zone_time| {
                let zone = zone_time.get()?.zone();
                zone_time.group_item(ClockTarget::ZoneTime(zone))
            }))
            .flatten(),
        )
    }
}

fn ticks() -> impl Stream<Item = ClockMessage> {
    stream::repeat_with(tick_at_next_boundary).then(identity)
}

async fn tick_at_next_boundary() -> ClockMessage {
    sleep(until_next_boundary(Local::now(), TICK)).await;
    ClockMessage::Tick(Local::now())
}

fn until_next_boundary(now: DateTime<Local>, period: Duration) -> Duration {
    let secs_into = now
        .timestamp()
        .rem_euclid(period.as_secs().cast_signed())
        .cast_unsigned();
    let subsec_ns = u64::from(now.timestamp_subsec_nanos()) % 1_000_000_000;
    Duration::from_secs(period.as_secs() - secs_into)
        .saturating_sub(Duration::from_nanos(subsec_ns))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn boundary_delay_completes_the_period() {
        let now = Local.timestamp_opt(1_000, 250_000_000).unwrap();
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(1)),
            Duration::from_millis(750)
        );
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(60)),
            Duration::from_millis(19750)
        );
    }

    #[test]
    fn exact_boundary_waits_a_full_period() {
        let now = Local.timestamp_opt(1_000, 0).unwrap();
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(1)),
            Duration::from_secs(1)
        );
    }
}
