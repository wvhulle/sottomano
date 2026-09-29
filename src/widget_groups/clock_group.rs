use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{Subscription, Task, futures::channel::mpsc::Sender, stream};
use tokio::time::sleep;

use crate::{
    config::Config,
    widgets::{
        group_item::GroupItem,
        localized_time::TimePart,
        text::bar_text,
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

    pub fn widget_group<'a>(&'a self, config: &'a Config) -> WidgetGroup<'a, ClockMessage> {
        WidgetGroup::new(&self.expansion, ClockMessage::Expansion)
            .item(GroupItem::element(move |_| {
                bar_text(
                    &config.font,
                    TimePart::TimeOfDay.format(&self.now, config.locale),
                )
                .into()
            }))
            .item(GroupItem::element(move |_| {
                bar_text(
                    &config.font,
                    TimePart::Date.format(&self.now, config.locale),
                )
                .into()
            }))
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
