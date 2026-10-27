use std::{
    convert::identity,
    future::{Future, ready},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use futures::{Stream, StreamExt, stream};
use tokio::time::{Instant as TokioClockInstant, MissedTickBehavior, interval, sleep};
use tokio_stream::wrappers::IntervalStream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    stream_operators::{StreamOperators, connected_until_failure, reread_on_trigger_or_deadline},
};

const SHORTEST_POLL_PERIOD: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodicPolling {
    poll_period: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryCadence {
    PeriodicPolling(PeriodicPolling),
    LivePushWithRecheck(PeriodicPolling),
    LivePush,
    SingleRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggeredRereadCadence {
    PeriodicPolling(PeriodicPolling),
    LivePushWithRecheck(PeriodicPolling),
    LivePush,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StampedCadence {
    delivery_cadence: DeliveryCadence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollTick {
    polling: PeriodicPolling,
}

pub struct AdaptiveCadence<CurrentCadence> {
    pub current_cadence: CurrentCadence,
}

impl PeriodicPolling {
    #[must_use]
    pub const fn every(poll_period: Duration) -> Self {
        if poll_period.as_nanos() < SHORTEST_POLL_PERIOD.as_nanos() {
            Self {
                poll_period: SHORTEST_POLL_PERIOD,
            }
        } else {
            Self { poll_period }
        }
    }

    #[must_use]
    pub fn poll_period(self) -> Duration {
        self.poll_period
    }

    pub fn poll_ticks(self) -> impl Stream<Item = PollTick> + Send {
        let mut timer = interval(self.poll_period);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        IntervalStream::new(timer).map(move |_| PollTick { polling: self })
    }

    pub fn poll_deliveries<S, F>(
        self,
        poll: impl Fn() -> F + Send + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<S>> + Send
    where
        S: SnapshotSource + PartialEq,
        F: Future<Output = Result<S, S::Error>> + Send,
    {
        self.poll_ticks()
            .then(move |poll_tick| {
                let poll = poll.clone();
                async move { poll_tick.stamped(poll().await) }
            })
            .repeats_as_unchanged()
    }

    pub fn poll_deliveries_after_setup<S, P, F>(
        self,
        setup: impl Future<Output = Result<P, S::Error>> + Send,
        poll: impl Fn(P) -> F + Send + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<S>> + Send
    where
        S: SnapshotSource + PartialEq,
        P: Clone + Send + 'static,
        F: Future<Output = Result<S, S::Error>> + Send,
    {
        stream::once(setup).flat_map(move |prepared| match prepared {
            Ok(prepared) => {
                let poll = poll.clone();
                self.poll_deliveries(move || poll(prepared.clone()))
                    .left_stream()
            }
            Err(failure) => {
                stream::once(ready(SnapshotDelivery::single_read(Err(failure)))).right_stream()
            }
        })
    }

    pub fn boundary_aligned_deliveries<S: SnapshotSource>(
        self,
        snapshot: impl Fn() -> S + Send + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<S>> + Send {
        stream::repeat_with(move || {
            let snapshot = snapshot.clone();
            async move {
                sleep(until_next_boundary(Local::now(), self.poll_period)).await;
                PollTick { polling: self }.stamped(Ok(snapshot()))
            }
        })
        .then(identity)
    }
}

impl PollTick {
    pub fn stamped<S: SnapshotSource>(
        self,
        snapshot_fetch: Result<S, S::Error>,
    ) -> SnapshotDelivery<S> {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            stamped_cadence: StampedCadence {
                delivery_cadence: DeliveryCadence::PeriodicPolling(self.polling),
            },
        }
    }
}

impl StampedCadence {
    #[must_use]
    pub fn delivery_cadence(self) -> DeliveryCadence {
        self.delivery_cadence
    }

    fn recheck_deadline(self) -> Option<Instant> {
        match self.delivery_cadence {
            DeliveryCadence::PeriodicPolling(polling)
            | DeliveryCadence::LivePushWithRecheck(polling) => {
                Some(TokioClockInstant::now().into_std() + polling.poll_period)
            }
            DeliveryCadence::LivePush | DeliveryCadence::SingleRead => None,
        }
    }
}

impl From<TriggeredRereadCadence> for DeliveryCadence {
    fn from(cadence: TriggeredRereadCadence) -> Self {
        match cadence {
            TriggeredRereadCadence::PeriodicPolling(polling) => {
                DeliveryCadence::PeriodicPolling(polling)
            }
            TriggeredRereadCadence::LivePushWithRecheck(polling) => {
                DeliveryCadence::LivePushWithRecheck(polling)
            }
            TriggeredRereadCadence::LivePush => DeliveryCadence::LivePush,
        }
    }
}

impl TriggeredRereadCadence {
    pub fn triggered_deliveries<S, F>(
        self,
        triggers: impl Stream<Item = ()> + Send + 'static,
        read: impl FnMut() -> F + Send + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<S>> + Send
    where
        S: SnapshotSource,
        F: Future<Output = Result<S, S::Error>> + Send + 'static,
    {
        AdaptiveCadence {
            current_cadence: move || self,
        }
        .triggered_deliveries(triggers, read)
    }
}

impl<CurrentCadence> AdaptiveCadence<CurrentCadence>
where
    CurrentCadence: Fn() -> TriggeredRereadCadence + Send + Sync + 'static,
{
    pub fn triggered_deliveries<S, F>(
        self,
        triggers: impl Stream<Item = ()> + Send + 'static,
        mut read: impl FnMut() -> F + Send + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<S>> + Send
    where
        S: SnapshotSource,
        F: Future<Output = Result<S, S::Error>> + Send + 'static,
    {
        let current_cadence = Arc::new(self.current_cadence);
        reread_on_trigger_or_deadline(
            triggers,
            move || {
                let fetching = read();
                let current_cadence = current_cadence.clone();
                async move {
                    let snapshot_fetch = fetching.await;
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch,
                        stamped_cadence: StampedCadence {
                            delivery_cadence: current_cadence().into(),
                        },
                    }
                }
            },
            |delivery| match delivery {
                SnapshotDelivery::SnapshotDelivered {
                    stamped_cadence, ..
                } => stamped_cadence.recheck_deadline(),
                SnapshotDelivery::SourceUnavailable | SnapshotDelivery::SnapshotUnchanged => None,
            },
        )
    }
}

impl<S: SnapshotSource> SnapshotDelivery<S> {
    pub fn live_push(snapshot_fetch: Result<S, S::Error>) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            stamped_cadence: StampedCadence {
                delivery_cadence: DeliveryCadence::LivePush,
            },
        }
    }

    pub fn single_read(snapshot_fetch: Result<S, S::Error>) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            stamped_cadence: StampedCadence {
                delivery_cadence: DeliveryCadence::SingleRead,
            },
        }
    }

    pub fn live_push_deliveries<R>(
        connect: impl Future<Output = Result<R, S::Error>> + Send,
    ) -> impl Stream<Item = Self> + Send
    where
        S: PartialEq,
        R: Stream<Item = Result<S, S::Error>> + Send,
    {
        connected_until_failure(connect)
            .map(SnapshotDelivery::live_push)
            .distinct_until_changed()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn fixture_delivered(
        snapshot_fetch: Result<S, S::Error>,
        delivery_cadence: DeliveryCadence,
    ) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            stamped_cadence: StampedCadence { delivery_cadence },
        }
    }

    pub fn single_read_deliveries(
        read: impl Future<Output = Result<S, S::Error>> + Send,
    ) -> impl Stream<Item = Self> + Send {
        stream::once(read).map(SnapshotDelivery::single_read)
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
    use std::{
        fmt::{Display, Formatter, Result as FmtResult},
        sync::atomic::{AtomicBool, Ordering},
    };

    use chrono::TimeZone;
    use futures::{StreamExt, stream};

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct CountedSnapshot(u32);

    #[derive(Debug, Clone)]
    struct NeverFails;

    impl Display for NeverFails {
        fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
            write!(f, "never fails")
        }
    }

    impl SnapshotSource for CountedSnapshot {
        type Error = NeverFails;
        type StreamInputs = ();

        const NAME: &'static str = "counted-snapshot";

        fn delivery_stream(_inputs: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
            stream::empty()
        }
    }

    fn stamped_cadence_of(delivery: &SnapshotDelivery<CountedSnapshot>) -> DeliveryCadence {
        let SnapshotDelivery::SnapshotDelivered {
            stamped_cadence, ..
        } = delivery
        else {
            panic!("expected a delivered snapshot, got {delivery:?}");
        };
        stamped_cadence.delivery_cadence()
    }

    #[test]
    fn a_zero_poll_period_is_raised_to_the_shortest_period() {
        assert_eq!(
            PeriodicPolling::every(Duration::ZERO).poll_period(),
            SHORTEST_POLL_PERIOD
        );
        assert_eq!(
            PeriodicPolling::every(Duration::from_secs(5)).poll_period(),
            Duration::from_secs(5)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn polled_deliveries_carry_their_period_and_repeats_arrive_unchanged() {
        let polling = PeriodicPolling::every(Duration::from_secs(3));
        let deliveries: Vec<_> = polling
            .poll_deliveries(|| ready(Ok(CountedSnapshot(7))))
            .take(2)
            .collect()
            .await;
        assert_eq!(
            stamped_cadence_of(&deliveries[0]),
            DeliveryCadence::PeriodicPolling(polling)
        );
        assert!(matches!(deliveries[1], SnapshotDelivery::SnapshotUnchanged));
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_unavailability_is_never_reported_unchanged() {
        let deliveries: Vec<SnapshotDelivery<CountedSnapshot>> = stream::iter([
            SnapshotDelivery::SourceUnavailable,
            SnapshotDelivery::SourceUnavailable,
        ])
        .repeats_as_unchanged()
        .collect()
        .await;
        assert!(
            deliveries
                .iter()
                .all(|delivery| matches!(delivery, SnapshotDelivery::SourceUnavailable))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn an_adaptive_cadence_stamps_and_schedules_from_the_same_value() {
        let push_connected = Arc::new(AtomicBool::new(false));
        let polling = PeriodicPolling::every(Duration::from_mins(1));
        let recheck = PeriodicPolling::every(Duration::from_mins(15));
        let cadence_switch = push_connected.clone();
        let adaptive = AdaptiveCadence {
            current_cadence: move || {
                if cadence_switch.load(Ordering::Relaxed) {
                    TriggeredRereadCadence::LivePushWithRecheck(recheck)
                } else {
                    TriggeredRereadCadence::PeriodicPolling(polling)
                }
            },
        };
        let read_switch = push_connected.clone();
        let mut deliveries =
            Box::pin(adaptive.triggered_deliveries(stream::pending(), move || {
                read_switch.store(true, Ordering::Relaxed);
                ready(Ok(CountedSnapshot(1)))
            }));
        let first = deliveries.next().await.unwrap();
        assert_eq!(
            stamped_cadence_of(&first),
            DeliveryCadence::LivePushWithRecheck(recheck)
        );
        let started_waiting = TokioClockInstant::now();
        deliveries.next().await.unwrap();
        assert_eq!(started_waiting.elapsed(), recheck.poll_period());
    }

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
