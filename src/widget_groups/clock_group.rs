mod clock_reading;
mod local_date;
mod local_time;
mod zone_time;

use std::time::Duration;

use chrono::{DateTime, Local};
use chrono_tz::Tz;
use iced::{Subscription, Task, futures::channel::mpsc::Sender, stream};
use tokio::time::sleep;

use self::{local_date::LocalDate, local_time::LocalTime, zone_time::ZoneTime};
use crate::{
    config::Config,
    desktop_shell::hover_tooltips::HoverTarget,
    widgets::{
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

const TICK: Duration = Duration::from_secs(1);

pub struct ClockGroup {
    now: DateTime<Local>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone, Copy)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
    Expansion(ExpansionMessage),
}

impl Default for ClockGroup {
    fn default() -> Self {
        Self {
            now: Local::now(),
            expansion: GroupExpansion::default(),
        }
    }
}

impl ClockGroup {
    pub fn update(&mut self, msg: ClockMessage) -> Task<ClockMessage> {
        match msg {
            ClockMessage::Tick(now) => self.now = now,
            ClockMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn subscription() -> Subscription<ClockMessage> {
        use iced::futures::SinkExt;
        Subscription::run(|| {
            stream::channel(1, |mut output: Sender<ClockMessage>| async move {
                loop {
                    sleep(until_next_boundary(Local::now(), TICK)).await;
                    if output.send(ClockMessage::Tick(Local::now())).await.is_err() {
                        return;
                    }
                }
            })
        })
    }

    fn zone_time(&self, zone: Tz) -> ZoneTime {
        ZoneTime(self.now.with_timezone(&zone))
    }

    pub fn tooltip_for(&self, target: &HoverTarget, config: &Config) -> Option<Tooltip> {
        match target {
            HoverTarget::LocalTime => Some(LocalTime(self.now).tooltip(config.locale)),
            HoverTarget::LocalDate => Some(LocalDate(self.now).tooltip(config.locale)),
            HoverTarget::ZoneTime(zone) => Some(self.zone_time(*zone).tooltip(config.locale)),
            _ => None,
        }
    }

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, ClockMessage> {
        WidgetGroup::new(&self.expansion, ClockMessage::Expansion)
            .item(LocalTime(self.now).reading(config.locale).item(config))
            .item(LocalDate(self.now).reading(config.locale).item(config))
            .items(
                config
                    .other_time_zones
                    .iter()
                    .map(|&zone| self.zone_time(zone).reading(config.locale).item(config)),
            )
    }
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
