use std::time::Duration;

use chrono::{DateTime, Local};
use iced::{Element, Subscription, futures::channel::mpsc::Sender, stream};
use tokio::time::sleep;

use crate::{config::resolved::ResolvedConfig, widgets::text::bar_text};

/// The strftime format the clock renders. The tick period is derived from it
/// ([`period`]), so changing the precision here adjusts the redraw rate too.
const FORMAT: &str = "%H:%M:%S";

pub struct ClockGroup {
    text: String,
}

impl Default for ClockGroup {
    fn default() -> Self {
        Self {
            text: format_now(Local::now()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ClockMessage {
    Tick(DateTime<Local>),
}

impl ClockGroup {
    pub fn update(&mut self, msg: ClockMessage) {
        let ClockMessage::Tick(now) = msg;
        self.text = format_now(now);
    }

    /// Ticks just after each wall-clock boundary of [`period`] — a fixed
    /// cadence from subscription start (`iced::time::every`) would leave the
    /// display lagging real time by up to a whole period.
    pub fn subscription() -> Subscription<ClockMessage> {
        use iced::futures::SinkExt;
        Subscription::run(|| {
            stream::channel(1, |mut output: Sender<ClockMessage>| async move {
                loop {
                    sleep(until_next_boundary(Local::now(), period(FORMAT))).await;
                    if output.send(ClockMessage::Tick(Local::now())).await.is_err() {
                        return;
                    }
                }
            })
        })
    }

    pub fn view<'a>(&self, config: &ResolvedConfig) -> Element<'a, ClockMessage> {
        bar_text(&config.font, self.text.clone()).into()
    }
}

fn format_now(now: DateTime<Local>) -> String {
    now.format(FORMAT).to_string()
}

/// The coarsest tick period that keeps `format` current: every second only
/// when a seconds field is rendered, else every minute.
fn period(format: &str) -> Duration {
    let has_seconds = ["%S", "%T", "%X", "%r", "%s"]
        .iter()
        .any(|spec| format.contains(spec));
    if has_seconds {
        Duration::from_secs(1)
    } else {
        Duration::from_secs(60)
    }
}

/// Time left until `now`'s next wall-clock multiple of `period`. Derived from
/// the UTC timestamp: local offsets are whole minutes, so second and minute
/// boundaries coincide with UTC's.
fn until_next_boundary(now: DateTime<Local>, period: Duration) -> Duration {
    let secs_into = now
        .timestamp()
        .rem_euclid(period.as_secs().cast_signed())
        .cast_unsigned();
    // Modulo folds a leap second's nanos back into range.
    let subsec_ns = u64::from(now.timestamp_subsec_nanos()) % 1_000_000_000;
    // Never actually saturates: subsec_ns < 1 s ≤ the remaining whole seconds.
    Duration::from_secs(period.as_secs() - secs_into)
        .saturating_sub(Duration::from_nanos(subsec_ns))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn seconds_format_ticks_every_second() {
        assert_eq!(period("%H:%M:%S"), Duration::from_secs(1));
    }

    #[test]
    fn minute_format_ticks_every_minute() {
        assert_eq!(period("%H:%M"), Duration::from_secs(60));
    }

    #[test]
    fn boundary_delay_completes_the_period() {
        let now = Local.timestamp_opt(1_000, 250_000_000).unwrap();
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(1)),
            Duration::from_nanos(750_000_000)
        );
        assert_eq!(
            until_next_boundary(now, Duration::from_secs(60)),
            Duration::from_nanos(19_750_000_000)
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
