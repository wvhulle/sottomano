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
    data::source::{ReadingSource, SourceDelivery},
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{first_delivery, source_subscription},
    widgets::{
        continuous_reading::ContinuousReading,
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
    time: ContinuousReading<LocalTime>,
    date: ContinuousReading<LocalDate>,
    zone_times: Vec<(Tz, ContinuousReading<ZoneTime>)>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum ClockMessage {
    Time(SourceDelivery<LocalTime>),
    Date(SourceDelivery<LocalDate>),
    Zone(Tz, SourceDelivery<ZoneTime>),
    Expansion(ExpansionMessage),
}

impl ClockGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn update(&mut self, msg: ClockMessage) -> Task<ClockMessage> {
        match msg {
            ClockMessage::Time(reading) => self.time.receive_delivery(reading),
            ClockMessage::Date(reading) => self.date.receive_delivery(reading),
            ClockMessage::Zone(zone, reading) => self.zone_time_mut(zone).receive_delivery(reading),
            ClockMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn subscription(config: &Config) -> Subscription<ClockMessage> {
        let zones = config.calendar.other_time_zones.iter().map(|zone| {
            source_subscription::<ZoneTime>(*zone)
                .with(*zone)
                .map(|(zone, reading)| ClockMessage::Zone(zone, reading))
        });
        Subscription::batch(
            [
                source_subscription::<LocalTime>(()).map(ClockMessage::Time),
                source_subscription::<LocalDate>(()).map(ClockMessage::Date),
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

    fn zone_time_mut(&mut self, zone: Tz) -> &mut ContinuousReading<ZoneTime> {
        let position = self
            .zone_times
            .iter()
            .position(|(known, _)| *known == zone)
            .unwrap_or_else(|| {
                self.zone_times.push((zone, ContinuousReading::default()));
                self.zone_times.len() - 1
            });
        &mut self.zone_times[position].1
    }

    fn zone_time(&self, zone: Tz) -> Option<&ContinuousReading<ZoneTime>> {
        self.zone_times
            .iter()
            .find(|(known, _)| *known == zone)
            .map(|(_, zone_time)| zone_time)
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
            .chain(
                self.zone_times
                    .iter()
                    .map(|(zone, zone_time)| zone_time.group_item(ClockTarget::ZoneTime(*zone))),
            )
            .flatten(),
        )
    }
}

pub fn polled_at_second_boundaries<S, F>(
    inputs: S::StreamInputs,
    poll: impl Fn(S::StreamInputs) -> F + Send + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource,
    F: Future<Output = Result<S, S::Error>> + Send,
{
    stream::repeat_with(move || {
        let inputs = inputs.clone();
        let poll = poll.clone();
        async move {
            sleep(until_next_boundary(Local::now(), TICK)).await;
            poll(inputs).await
        }
    })
    .then(identity)
    .map(SourceDelivery::Reading)
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
