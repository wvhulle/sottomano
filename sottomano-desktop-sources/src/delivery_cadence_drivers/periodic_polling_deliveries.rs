use std::{future::Future, time::Duration};

use chrono::{DateTime, Local};
use futures::{FutureExt, Stream, StreamExt, TryStreamExt, stream};
use tokio::time::{Instant as TokioClockInstant, MissedTickBehavior, interval, sleep};
use tokio_stream::wrappers::IntervalStream;

use super::{
    CarriedRead, ChangeTriggerStreams, DeliveryCadence, PeriodicPolling, RereadSchedule,
    SamplingNeed, SnapshotDeliveryStreams, triggered_deliveries_with_recheck, uncarried_reads,
};
use crate::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::{Panicked, blocking_poll},
};

pub(super) const SHORTEST_POLL_PERIOD: Duration = Duration::from_secs(1);

enum BlockingState<State> {
    Unbuilt,
    Built(State),
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
        PollFuture: Future<Output = Result<Source, Source::Error>> + Send + 'static,
    {
        self.poll_deliveries_carrying((), uncarried_reads(poll))
    }

    fn poll_deliveries_carrying<Source, State, PollFuture>(
        self,
        carried_state: State,
        poll: impl FnMut(State) -> PollFuture + Send + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource + PartialEq,
        State: Send + 'static,
        PollFuture: Future<Output = CarriedRead<State, Result<Source, Source::Error>>> + Send,
    {
        stream::pending::<Result<(), Source::Error>>()
            .paced_rereads_carrying(Duration::ZERO, carried_state, poll, move |_| {
                RereadSchedule::RecheckAt(TokioClockInstant::now().into_std() + self.poll_period)
            })
            .map(move |snapshot_fetch| SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch,
                delivery_cadence: DeliveryCadence::PeriodicPolling(self),
            })
            .repeats_as_unchanged()
    }

    pub fn blocking_poll_deliveries_carrying<Source, State>(
        self,
        fresh_state: impl Fn() -> State + Send + Sync + Clone + 'static,
        read: impl Fn(&mut State) -> Result<Source, Source::Error> + Send + Sync + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource + PartialEq,
        Source::Error: From<Panicked>,
        State: Send + 'static,
    {
        self.poll_deliveries_carrying(BlockingState::Unbuilt, move |blocking_state| {
            let fresh_state = fresh_state.clone();
            let read = read.clone();
            blocking_poll(move || {
                let mut state = match blocking_state {
                    BlockingState::Unbuilt => fresh_state(),
                    BlockingState::Built(state) => state,
                };
                let snapshot_fetch = read(&mut state);
                Ok(CarriedRead {
                    carried_state: BlockingState::Built(state),
                    snapshot_fetch,
                })
            })
            .map(|blocking_read| {
                blocking_read.unwrap_or_else(|Panicked| CarriedRead {
                    carried_state: BlockingState::Unbuilt,
                    snapshot_fetch: Err(Source::Error::from(Panicked)),
                })
            })
        })
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
        PollFuture: Future<Output = Result<Source, Source::Error>> + Send + 'static,
    {
        stream::once(setup)
            .map_ok(move |prepared| {
                let poll = poll.clone();
                self.poll_deliveries(move || poll(prepared.clone())).map(Ok)
            })
            .try_flatten()
            .map(|delivery| {
                delivery.unwrap_or_else(|failure| SnapshotDelivery::single_read(Err(failure)))
            })
    }

    pub fn boundary_aligned_deliveries<Source: SnapshotSource>(
        self,
        snapshot: impl Fn() -> Source + Send + Clone + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send {
        stream::unfold(snapshot, move |snapshot| async move {
            sleep(until_next_boundary(Local::now(), self.poll_period)).await;
            let delivered = SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(snapshot()),
                delivery_cadence: DeliveryCadence::PeriodicPolling(self),
            };
            Some((delivered, snapshot))
        })
    }

    pub fn triggered_deliveries<Source, TriggerFailure, ReadFuture>(
        self,
        triggers: impl Stream<Item = Result<(), TriggerFailure>> + Send + 'static,
        read: impl FnMut() -> ReadFuture + Send + 'static,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Source: SnapshotSource,
        TriggerFailure: Into<Source::Error>,
        ReadFuture: Future<Output = Result<Source, Source::Error>> + Send + 'static,
    {
        triggered_deliveries_with_recheck(
            triggers,
            Duration::ZERO,
            self,
            (),
            uncarried_reads(read),
            |_| SamplingNeed::KeepSampling,
        )
    }
}

pub(super) fn until_next_boundary(now: DateTime<Local>, period: Duration) -> Duration {
    let secs_into = now
        .timestamp()
        .rem_euclid(period.as_secs().cast_signed())
        .cast_unsigned();
    let subsec_ns = u64::from(now.timestamp_subsec_nanos()) % 1_000_000_000;
    Duration::from_secs(period.as_secs() - secs_into)
        .checked_sub(Duration::from_nanos(subsec_ns))
        .expect("at least a whole second remains before the next boundary")
}
