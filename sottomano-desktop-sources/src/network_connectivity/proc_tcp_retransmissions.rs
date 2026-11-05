use std::{
    collections::VecDeque,
    fs,
    future::ready,
    time::{Duration, Instant},
};

use futures::{Stream, StreamExt};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    SnapshotDelivery, SnapshotSource, SourceCaption, delivery_cadence_drivers::PeriodicPolling,
    stream_operators::StreamOperators, system_protocol_clients::sysfs_attribute_reads::Error,
};

const PROC_NET_SNMP: &str = "/proc/net/snmp";
const SAMPLE_POLLING: PeriodicPolling = PeriodicPolling::every(Duration::from_secs(2));
const WINDOW_SAMPLES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TcpRetransmissions {
    pub retransmitted_share: Ratio,
    pub retransmitted_per_second: f64,
    pub sent_per_second: f64,
    pub window: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpSegmentCounters {
    pub sent_segments: u64,
    pub retransmitted_segments: u64,
}

struct CounterSample {
    counters: TcpSegmentCounters,
    taken_at: Instant,
}

impl SnapshotSource for TcpRetransmissions {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "TCP retransmissions";
    const CAPTION: SourceCaption = SourceCaption::Captioned(
        "Share of TCP segments sent again because no acknowledgement came back, across all \
         connections over the last half minute.",
    );

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        SAMPLE_POLLING
            .poll_ticks()
            .scan(VecDeque::new(), |window_samples, poll_tick| {
                let delivered = match read_segment_counters() {
                    Ok(counters) => {
                        window_samples.push_back(CounterSample {
                            counters,
                            taken_at: Instant::now(),
                        });
                        if window_samples.len() > WINDOW_SAMPLES {
                            window_samples.pop_front();
                        }
                        retransmissions_over(window_samples)
                            .map(|readings| poll_tick.stamped(Ok(readings)))
                    }
                    Err(failure) => Some(poll_tick.stamped(Err(failure))),
                };
                ready(Some(delivered))
            })
            .filter_map(ready)
            .distinct_until_changed()
    }
}

fn retransmissions_over(window_samples: &VecDeque<CounterSample>) -> Option<TcpRetransmissions> {
    let oldest = window_samples.front()?;
    let newest = window_samples.back()?;
    let window = newest.taken_at.duration_since(oldest.taken_at);
    if window.is_zero() {
        return None;
    }
    Some(TcpRetransmissions::between(
        oldest.counters,
        newest.counters,
        window,
    ))
}

impl TcpRetransmissions {
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn between(
        earlier: TcpSegmentCounters,
        later: TcpSegmentCounters,
        window: Duration,
    ) -> Self {
        let sent = later.sent_segments.saturating_sub(earlier.sent_segments) as f64;
        let retransmitted = later
            .retransmitted_segments
            .saturating_sub(earlier.retransmitted_segments) as f64;
        let share = if sent > 0.0 {
            retransmitted / sent
        } else {
            0.0
        };
        let seconds = window.as_secs_f64();
        Self {
            retransmitted_share: Ratio::new::<ratio>(share.min(1.0)),
            retransmitted_per_second: retransmitted / seconds,
            sent_per_second: sent / seconds,
            window,
        }
    }
}

fn read_segment_counters() -> Result<TcpSegmentCounters, Error> {
    let snmp_text = fs::read_to_string(PROC_NET_SNMP)
        .map_err(|_| Error::Unreadable(PROC_NET_SNMP.to_owned()))?;
    parse_segment_counters(&snmp_text).ok_or_else(|| Error::Unparsable(PROC_NET_SNMP.to_owned()))
}

#[must_use]
pub fn parse_segment_counters(snmp_text: &str) -> Option<TcpSegmentCounters> {
    let mut tcp_lines = snmp_text
        .lines()
        .filter_map(|line| line.strip_prefix("Tcp:"));
    let field_names = tcp_lines.next()?.split_whitespace();
    let field_values = tcp_lines.next()?.split_whitespace();
    let counter_named = |wanted_name: &str| {
        field_names
            .clone()
            .zip(field_values.clone())
            .find(|(field_name, _)| *field_name == wanted_name)
            .and_then(|(_, field_value)| field_value.parse().ok())
    };
    Some(TcpSegmentCounters {
        sent_segments: counter_named("OutSegs")?,
        retransmitted_segments: counter_named("RetransSegs")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNMP_EXCERPT: &str = "\
Ip: Forwarding DefaultTTL
Ip: 1 64
Tcp: RtoAlgorithm RtoMin RtoMax MaxConn ActiveOpens PassiveOpens AttemptFails EstabResets \
                                CurrEstab InSegs OutSegs RetransSegs InErrs OutRsts InCsumErrors
Tcp: 1 200 120000 -1 155886 122383 7821 5633 81 4935763 11827475 19525 13 26973 0
Udp: InDatagrams NoPorts
Udp: 10 2
";

    #[test]
    fn tcp_counters_are_read_by_field_name() {
        assert_eq!(
            parse_segment_counters(SNMP_EXCERPT),
            Some(TcpSegmentCounters {
                sent_segments: 11_827_475,
                retransmitted_segments: 19_525,
            })
        );
    }

    #[test]
    fn missing_tcp_section_is_unparsable() {
        assert_eq!(parse_segment_counters("Udp: InDatagrams\nUdp: 1\n"), None);
    }

    #[test]
    fn share_is_retransmitted_over_sent_within_the_window() {
        let earlier = TcpSegmentCounters {
            sent_segments: 1_000,
            retransmitted_segments: 10,
        };
        let later = TcpSegmentCounters {
            sent_segments: 3_000,
            retransmitted_segments: 50,
        };
        let retransmissions = TcpRetransmissions::between(earlier, later, Duration::from_secs(20));
        assert!((retransmissions.retransmitted_share.get::<ratio>() - 0.02).abs() < 1e-9);
        assert!((retransmissions.sent_per_second - 100.0).abs() < 1e-9);
        assert!((retransmissions.retransmitted_per_second - 2.0).abs() < 1e-9);
    }

    #[test]
    fn idle_window_has_no_retransmitted_share() {
        let counters = TcpSegmentCounters {
            sent_segments: 5,
            retransmitted_segments: 1,
        };
        let retransmissions =
            TcpRetransmissions::between(counters, counters, Duration::from_secs(2));
        assert!(retransmissions.retransmitted_share.get::<ratio>().abs() < f64::EPSILON);
    }
}
