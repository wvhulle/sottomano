use std::{
    convert::identity,
    future::{Future, ready},
    time::Duration,
};

use chrono::{DateTime, Local};
use futures::{Stream, StreamExt, stream};
use tokio::time::{Instant as TokioClockInstant, MissedTickBehavior, interval, sleep};
use tokio_stream::wrappers::IntervalStream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    stream_operators::{
        RereadSchedule, StreamOperators, connected_until_failure,
        reread_on_spaced_trigger_or_deadline,
    },
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
pub enum SamplingNeed {
    KeepSampling,
    Settled,
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

    pub(crate) fn poll_ticks(self) -> impl Stream<Item = Self> + Send {
        let mut timer = interval(self.poll_period);
        timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        IntervalStream::new(timer).map(move |_| self)
    }

    pub fn poll_deliveries<Source, PollFuture>(
        self,
        poll: impl Fn() -> PollFuture + Send + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource + PartialEq,
        PollFuture: Future<Output = Result<Source, Source::Error>> + Send,
    {
        self.poll_ticks()
            .then(move |polling| {
                let poll = poll.clone();
                async move {
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: poll().await,
                        delivery_cadence: DeliveryCadence::PeriodicPolling(polling),
                    }
                }
            })
            .repeats_as_unchanged()
    }

    pub fn blocking_poll_deliveries<Source>(
        self,
        read: impl Fn() -> Result<Source, Source::Error> + Send + Sync + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource + PartialEq,
        Source::Error: From<Panicked>,
    {
        self.poll_deliveries(move || blocking_poll(read.clone()))
    }

    pub fn blocking_poll_deliveries_after_setup<Source, Prepared>(
        self,
        setup: impl Future<Output = Result<Prepared, Source::Error>> + Send,
        read: impl Fn(Prepared) -> Result<Source, Source::Error> + Send + Sync + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource + PartialEq,
        Source::Error: From<Panicked>,
        Prepared: Clone + Send + 'static,
    {
        self.poll_deliveries_after_setup(setup, move |prepared| {
            let read = read.clone();
            blocking_poll(move || read(prepared))
        })
    }

    pub fn poll_deliveries_after_setup<Source, Prepared, PollFuture>(
        self,
        setup: impl Future<Output = Result<Prepared, Source::Error>> + Send,
        poll: impl Fn(Prepared) -> PollFuture + Send + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource + PartialEq,
        Prepared: Clone + Send + 'static,
        PollFuture: Future<Output = Result<Source, Source::Error>> + Send,
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

    pub fn boundary_aligned_deliveries<Source: SnapshotSource>(
        self,
        snapshot: impl Fn() -> Source + Send + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send {
        stream::repeat_with(move || {
            let snapshot = snapshot.clone();
            async move {
                sleep(until_next_boundary(Local::now(), self.poll_period)).await;
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(snapshot()),
                    delivery_cadence: DeliveryCadence::PeriodicPolling(self),
                }
            }
        })
        .then(identity)
    }

    pub fn triggered_deliveries<Source, ReadFuture>(
        self,
        triggers: impl Stream<Item = ()> + Send + 'static,
        read: impl FnMut() -> ReadFuture + Send + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource,
        ReadFuture: Future<Output = Result<Source, Source::Error>> + Send + 'static,
    {
        triggered_deliveries_with_recheck(triggers, Duration::ZERO, self, read, |_| {
            SamplingNeed::KeepSampling
        })
    }
}

pub fn triggered_deliveries_with_recheck<Source, ReadFuture>(
    triggers: impl Stream<Item = ()> + Send + 'static,
    minimum_trigger_gap: Duration,
    recheck: PeriodicPolling,
    mut read: impl FnMut() -> ReadFuture + Send + 'static,
    sampling_need: fn(&Result<Source, Source::Error>) -> SamplingNeed,
) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
where
    Source: SnapshotSource,
    ReadFuture: Future<Output = Result<Source, Source::Error>> + Send + 'static,
{
    reread_on_spaced_trigger_or_deadline(
        triggers,
        minimum_trigger_gap,
        move || {
            let fetching = read();
            async move {
                let snapshot_fetch = fetching.await;
                let delivery_cadence = match sampling_need(&snapshot_fetch) {
                    SamplingNeed::KeepSampling => DeliveryCadence::LivePushWithRecheck(recheck),
                    SamplingNeed::Settled => DeliveryCadence::LivePush,
                };
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch,
                    delivery_cadence,
                }
            }
        },
        move |delivery| match delivery {
            SnapshotDelivery::SnapshotDelivered {
                delivery_cadence: DeliveryCadence::LivePushWithRecheck(polling),
                ..
            } => {
                RereadSchedule::RecheckAt(TokioClockInstant::now().into_std() + polling.poll_period)
            }
            SnapshotDelivery::SnapshotDelivered { .. }
            | SnapshotDelivery::SourceUnavailable
            | SnapshotDelivery::InternetUnreachable(_)
            | SnapshotDelivery::SnapshotUnchanged => RereadSchedule::WaitForTrigger,
        },
    )
}

impl<Source: SnapshotSource> SnapshotDelivery<Source> {
    pub fn live_push(snapshot_fetch: Result<Source, Source::Error>) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            delivery_cadence: DeliveryCadence::LivePush,
        }
    }

    pub fn single_read(snapshot_fetch: Result<Source, Source::Error>) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            delivery_cadence: DeliveryCadence::SingleRead,
        }
    }

    pub fn live_push_deliveries<ReadStream>(
        connect: impl Future<Output = Result<ReadStream, Source::Error>> + Send,
    ) -> impl Stream<Item = Self> + Send
    where
        Source: PartialEq,
        ReadStream: Stream<Item = Result<Source, Source::Error>> + Send,
    {
        connected_until_failure(connect)
            .map(SnapshotDelivery::live_push)
            .distinct_until_changed()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn fixture_delivered(
        snapshot_fetch: Result<Source, Source::Error>,
        delivery_cadence: DeliveryCadence,
    ) -> Self {
        SnapshotDelivery::SnapshotDelivered {
            snapshot_fetch,
            delivery_cadence,
        }
    }

    pub fn single_read_deliveries(
        read: impl Future<Output = Result<Source, Source::Error>> + Send,
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
    use std::fmt::{Display, Formatter, Result as FmtResult};

    use chrono::TimeZone;
    use futures::{StreamExt, stream};

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct CountedSnapshot(u32);

    #[derive(Debug, Clone)]
    struct NeverFails;

    impl Display for NeverFails {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
            write!(formatter, "never fails")
        }
    }

    impl SnapshotSource for CountedSnapshot {
        type Error = NeverFails;
        type StreamInputs = ();

        const NAME: &'static str = "Counted snapshot";

        fn delivery_stream(_inputs: ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
            stream::empty()
        }
    }

    fn delivery_cadence_of(delivery: &SnapshotDelivery<CountedSnapshot>) -> DeliveryCadence {
        let SnapshotDelivery::SnapshotDelivered {
            delivery_cadence, ..
        } = delivery
        else {
            panic!("expected a delivered snapshot, got {delivery:?}");
        };
        *delivery_cadence
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
            delivery_cadence_of(&deliveries[0]),
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
    async fn triggered_deliveries_recheck_on_the_polling_period() {
        let recheck = PeriodicPolling::every(Duration::from_mins(15));
        let mut deliveries = Box::pin(
            recheck.triggered_deliveries(stream::pending(), || ready(Ok(CountedSnapshot(1)))),
        );
        let first = deliveries.next().await.unwrap();
        assert_eq!(
            delivery_cadence_of(&first),
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
