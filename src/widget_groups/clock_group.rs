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
    desktop_shell::{hover_tooltips::HoverTarget, monitor_surfaces::Position},
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, GroupHandle, HandleFace, WidgetGroup},
    },
};

const TICK: Duration = Duration::from_secs(1);

pub struct ClockGroup {
    time: LocalTime,
    date: LocalDate,
    zone_times: Vec<ZoneTime>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone, Copy)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
    Expansion(ExpansionMessage),
}

impl Default for ClockGroup {
    fn default() -> Self {
        let now = Local::now();
        Self {
            time: LocalTime(now),
            date: LocalDate(now),
            zone_times: Vec::new(),
            expansion: GroupExpansion::default(),
        }
    }
}

impl ClockGroup {
    pub fn update(&mut self, msg: ClockMessage, config: &Config) -> Task<ClockMessage> {
        match msg {
            ClockMessage::Tick(now) => self.tick(now, &config.other_time_zones),
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
        self.time = LocalTime(now);
        self.date = LocalDate(now);
        self.zone_times = other_time_zones
            .iter()
            .map(|zone| ZoneTime(now.with_timezone(zone)))
            .collect();
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        let body = match target {
            HoverTarget::LocalTime => self.time.tooltip(),
            HoverTarget::LocalDate => self.date.tooltip(),
            HoverTarget::ZoneTime(zone) => self
                .zone_times
                .iter()
                .find(|zone_time| zone_time.zone() == *zone)?
                .tooltip(),
            _ => return None,
        };
        Some(Tooltip::of(body))
    }

    pub fn widget_group<'a, Outer: 'a>(&'a self, config: &'a Config, position: Position, lift: fn(ClockMessage) -> Outer) -> WidgetGroup<'a, Outer> {
        let zone_items = self.zone_times.iter().map(|zone_time| {
            GroupItem::reading(zone_time, HoverTarget::ZoneTime(zone_time.zone()))
        });
        let items: Vec<_> = [
            GroupItem::reading(&self.time, HoverTarget::LocalTime),
            GroupItem::reading(&self.date, HoverTarget::LocalDate),
        ]
        .into_iter()
        .chain(zone_items)
        .collect();
        WidgetGroup::new(
            &self.expansion,
            ClockMessage::Expansion,
            GroupHandle {
                face: HandleFace::FirstItem,
                preview: None,
                forwards_scroll: false,
            },
            &items,
            config,
            position,
            lift,
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
