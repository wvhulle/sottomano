use std::{
    convert::identity,
    future::{Future, ready},
    pin::Pin,
    task::{Context, Poll, ready as ready_poll},
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use futures::{
    Stream, StreamExt,
    stream::{self, Fuse, FusedStream},
};
use futures_rx::RxExt;
use pin_project_lite::pin_project;
use tokio::time::{
    Instant as TokioClockInstant, MissedTickBehavior, Sleep, interval, sleep, sleep_until,
};
use tokio_stream::wrappers::IntervalStream;

use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
    system_protocol_clients::protocol_peer_reconnects::subscribed,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RereadSchedule {
    WaitForTrigger,
    RecheckAt(Instant),
}

pub trait ChangeTriggerStreams: Stream<Item = ()> + Sized + Send + 'static {
    fn triggered_rereads<Produced, ReadFuture>(
        self,
        read: impl FnMut() -> ReadFuture + Send + 'static,
    ) -> impl Stream<Item = Produced> + Send
    where
        Produced: Send + 'static,
        ReadFuture: Future<Output = Produced> + Send,
    {
        self.paced_rereads(Duration::ZERO, read, |_| RereadSchedule::WaitForTrigger)
    }

    fn paced_rereads<Produced, ReadFuture>(
        self,
        minimum_trigger_gap: Duration,
        read: impl FnMut() -> ReadFuture + Send + 'static,
        recheck_after: impl Fn(&Produced) -> RereadSchedule + Send + 'static,
    ) -> impl Stream<Item = Produced> + Send
    where
        Produced: Send + 'static,
        ReadFuture: Future<Output = Produced> + Send,
    {
        RereadOnTrigger {
            triggers: self.fuse(),
            minimum_trigger_gap,
            recheck_after,
            read,
            reading: None,
            cooldown: None,
            deadline: None,
            triggered: true,
        }
    }
}

impl<TriggerStream: Stream<Item = ()> + Send + 'static> ChangeTriggerStreams for TriggerStream {}

pin_project! {
    struct RereadOnTrigger<TriggerStream, RecheckFn, ReadFn, ReadFuture> {
        #[pin]
        triggers: Fuse<TriggerStream>,
        minimum_trigger_gap: Duration,
        recheck_after: RecheckFn,
        read: ReadFn,
        #[pin]
        reading: Option<ReadFuture>,
        #[pin]
        cooldown: Option<Sleep>,
        #[pin]
        deadline: Option<Sleep>,
        triggered: bool,
    }
}

impl<TriggerStream, RecheckFn, ReadFn, ReadFuture> Stream
    for RereadOnTrigger<TriggerStream, RecheckFn, ReadFn, ReadFuture>
where
    TriggerStream: Stream<Item = ()>,
    RecheckFn: Fn(&ReadFuture::Output) -> RereadSchedule,
    ReadFn: FnMut() -> ReadFuture,
    ReadFuture: Future,
{
    type Item = ReadFuture::Output;

    fn poll_next(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<ReadFuture::Output>> {
        let mut projected = self.project();
        while let Poll::Ready(Some(())) = projected.triggers.as_mut().poll_next(context) {
            *projected.triggered = true;
        }
        if let Some(cooldown) = projected.cooldown.as_mut().as_pin_mut()
            && cooldown.poll(context).is_ready()
        {
            projected.cooldown.set(None);
        }
        if let Some(deadline) = projected.deadline.as_mut().as_pin_mut()
            && deadline.poll(context).is_ready()
        {
            *projected.triggered = true;
            projected.deadline.set(None);
            projected.cooldown.set(None);
        }
        if projected.reading.is_none() {
            if !*projected.triggered {
                return if projected.triggers.is_terminated() && projected.deadline.is_none() {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                };
            }
            if projected.cooldown.is_some() {
                return Poll::Pending;
            }
            *projected.triggered = false;
            if !projected.minimum_trigger_gap.is_zero() {
                projected
                    .cooldown
                    .set(Some(sleep(*projected.minimum_trigger_gap)));
                if let Some(cooldown) = projected.cooldown.as_mut().as_pin_mut()
                    && cooldown.poll(context).is_ready()
                {
                    projected.cooldown.set(None);
                }
            }
            projected.reading.set(Some((projected.read)()));
        }
        let Some(reading) = projected.reading.as_mut().as_pin_mut() else {
            return Poll::Pending;
        };
        let produced = ready_poll!(reading.poll(context));
        projected.reading.set(None);
        let next_deadline = match (projected.recheck_after)(&produced) {
            RereadSchedule::WaitForTrigger => None,
            RereadSchedule::RecheckAt(recheck_instant) => Some(sleep_until(recheck_instant.into())),
        };
        projected.deadline.set(next_deadline);
        Poll::Ready(Some(produced))
    }
}

pub trait SnapshotDeliveryStreams<Source: SnapshotSource>:
    Stream<Item = SnapshotDelivery<Source>> + Sized
{
    fn repeats_as_unchanged(self) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Self: Send,
        Source: PartialEq,
    {
        self.scan(
            None,
            |previous: &mut Option<SnapshotDelivery<Source>>, delivery| {
                let repeated_snapshot = matches!(
                    delivery,
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Ok(_),
                        ..
                    }
                ) && previous.as_ref() == Some(&delivery);
                *previous = Some(delivery.clone());
                ready(Some(if repeated_snapshot {
                    SnapshotDelivery::SnapshotUnchanged
                } else {
                    delivery
                }))
            },
        )
    }
}

impl<Source: SnapshotSource, DeliveryStream: Stream<Item = SnapshotDelivery<Source>>>
    SnapshotDeliveryStreams<Source> for DeliveryStream
{
}

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
    triggers.paced_rereads(
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
        |delivery| match delivery {
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

    pub fn live_push_deliveries<ConnectInputs, ReadStream, ConnectFuture>(
        connect_inputs: ConnectInputs,
        connect: impl FnMut(ConnectInputs) -> ConnectFuture + Send,
    ) -> impl Stream<Item = Self> + Send
    where
        ConnectInputs: Clone + Send,
        Source: PartialEq,
        ReadStream: Stream<Item = Result<Source, Source::Error>> + Send,
        ConnectFuture: Future<Output = Result<ReadStream, Source::Error>> + Send,
    {
        subscribed(connect_inputs, connect)
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
    async fn spaced_rereads_drain_burst_triggers_and_wait_the_gap() {
        let minimum_trigger_gap = Duration::from_mins(1);
        let started = TokioClockInstant::now();
        let (trigger_sender, trigger_receiver) = futures::channel::mpsc::unbounded();
        let mut read_count = 0_u32;
        let mut rereads = Box::pin(trigger_receiver.paced_rereads(
            minimum_trigger_gap,
            move || {
                read_count += 1;
                ready(read_count)
            },
            |_| RereadSchedule::WaitForTrigger,
        ));
        assert_eq!(rereads.next().await, Some(1));
        assert_eq!(started.elapsed(), Duration::ZERO);
        trigger_sender.unbounded_send(()).unwrap();
        trigger_sender.unbounded_send(()).unwrap();
        assert_eq!(rereads.next().await, Some(2));
        assert_eq!(started.elapsed(), minimum_trigger_gap);
        assert!(
            tokio::time::timeout(minimum_trigger_gap * 10, rereads.next())
                .await
                .is_err()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn triggers_drain_while_a_read_is_pending() {
        let (trigger_sender, trigger_receiver) = futures::channel::mpsc::channel::<()>(4);
        let mut rereads =
            Box::pin(trigger_receiver.triggered_rereads(futures::future::pending::<()>));
        let pending_read = tokio::spawn(async move { rereads.next().await });
        let sent_triggers = tokio::time::timeout(
            Duration::from_secs(1),
            stream::repeat(()).take(200).map(Ok).forward(trigger_sender),
        )
        .await;
        pending_read.abort();
        assert!(
            matches!(sent_triggers, Ok(Ok(()))),
            "triggers backed up while a read was pending"
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
