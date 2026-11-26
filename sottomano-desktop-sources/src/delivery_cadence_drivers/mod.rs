#[cfg(test)]
mod delivery_cadence_tests;
mod periodic_polling_deliveries;
mod snapshot_delivery_constructors;

use std::{
    collections::VecDeque,
    future::{Future, ready},
    io,
    pin::Pin,
    task::{Context, Poll, ready as ready_poll},
    time::{Duration, Instant},
};

use futures::{
    Stream, StreamExt,
    stream::{Fuse, FusedStream},
};
use pin_project_lite::pin_project;
use tokio::time::{Instant as TokioClockInstant, Sleep, sleep, sleep_until};

use crate::{SnapshotDelivery, SnapshotSource};

#[derive(Debug, thiserror::Error)]
pub enum ChangeWatchFailure {
    #[error("file watch stopped: {0}")]
    FileWatch(#[from] io::Error),
    #[error("signal subscription stopped: {0}")]
    SignalSubscription(#[from] zbus::Error),
}

impl PartialEq for ChangeWatchFailure {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::FileWatch(watch_failure), Self::FileWatch(other_failure)) => {
                watch_failure.kind() == other_failure.kind()
            }
            (
                Self::SignalSubscription(subscription_failure),
                Self::SignalSubscription(other_failure),
            ) => subscription_failure == other_failure,
            (Self::FileWatch(_), Self::SignalSubscription(_))
            | (Self::SignalSubscription(_), Self::FileWatch(_)) => false,
        }
    }
}

pub struct CarriedRead<State, Fetch> {
    pub carried_state: State,
    pub snapshot_fetch: Fetch,
}

pin_project! {
    struct UncarriedRead<ReadFuture> {
        #[pin]
        read_future: ReadFuture,
    }
}

impl<ReadFuture: Future> Future for UncarriedRead<ReadFuture> {
    type Output = CarriedRead<(), ReadFuture::Output>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.project()
            .read_future
            .poll(context)
            .map(|snapshot_fetch| CarriedRead {
                carried_state: (),
                snapshot_fetch,
            })
    }
}

fn uncarried_reads<ReadFuture>(
    mut read: impl FnMut() -> ReadFuture,
) -> impl FnMut(()) -> UncarriedRead<ReadFuture> {
    move |()| UncarriedRead {
        read_future: read(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RereadSchedule {
    WaitForTrigger,
    RecheckAt(Instant),
}

pub trait ChangeTriggerStreams<TriggerFailure>:
    Stream<Item = Result<(), TriggerFailure>> + Sized + Send + 'static
{
    fn triggered_rereads<Sample, Failure, ReadFuture>(
        self,
        read: impl FnMut() -> ReadFuture + Send + 'static,
    ) -> impl Stream<Item = Result<Sample, Failure>> + Send
    where
        Sample: Send + 'static,
        Failure: Send + 'static,
        TriggerFailure: Into<Failure>,
        ReadFuture: Future<Output = Result<Sample, Failure>> + Send + 'static,
    {
        self.paced_rereads(Duration::ZERO, read, |_| RereadSchedule::WaitForTrigger)
    }

    fn paced_rereads<Sample, Failure, ReadFuture>(
        self,
        minimum_trigger_gap: Duration,
        read: impl FnMut() -> ReadFuture + Send + 'static,
        recheck_after: impl Fn(&Result<Sample, Failure>) -> RereadSchedule + Send + 'static,
    ) -> impl Stream<Item = Result<Sample, Failure>> + Send
    where
        Sample: Send + 'static,
        Failure: Send + 'static,
        TriggerFailure: Into<Failure>,
        ReadFuture: Future<Output = Result<Sample, Failure>> + Send + 'static,
    {
        self.paced_rereads_carrying(
            minimum_trigger_gap,
            (),
            uncarried_reads(read),
            recheck_after,
        )
    }

    fn paced_rereads_carrying<State, Sample, Failure, ReadFuture>(
        self,
        minimum_trigger_gap: Duration,
        carried_state: State,
        read: impl FnMut(State) -> ReadFuture + Send + 'static,
        recheck_after: impl Fn(&Result<Sample, Failure>) -> RereadSchedule + Send + 'static,
    ) -> impl Stream<Item = Result<Sample, Failure>> + Send
    where
        State: Send + 'static,
        Sample: Send + 'static,
        Failure: Send + 'static,
        TriggerFailure: Into<Failure>,
        ReadFuture: Future<Output = CarriedRead<State, Result<Sample, Failure>>> + Send,
    {
        RereadOnTrigger {
            triggers: self.fuse(),
            minimum_trigger_gap,
            recheck_after,
            read,
            carried_state: Some(carried_state),
            trigger_failures: VecDeque::new(),
            sample: None,
            cooldown: None,
            deadline: None,
            triggered: true,
        }
    }
}

impl<TriggerFailure, TriggerStream> ChangeTriggerStreams<TriggerFailure> for TriggerStream where
    TriggerStream: Stream<Item = Result<(), TriggerFailure>> + Send + 'static
{
}

pin_project! {
    struct RereadOnTrigger<TriggerStream, Failure, State, RecheckFn, ReadFn, ReadFuture> {
        #[pin]
        triggers: Fuse<TriggerStream>,
        minimum_trigger_gap: Duration,
        recheck_after: RecheckFn,
        read: ReadFn,
        carried_state: Option<State>,
        trigger_failures: VecDeque<Failure>,
        #[pin]
        sample: Option<ReadFuture>,
        #[pin]
        cooldown: Option<Sleep>,
        #[pin]
        deadline: Option<Sleep>,
        triggered: bool,
    }
}

impl<TriggerStream, TriggerFailure, Failure, State, Sample, RecheckFn, ReadFn, ReadFuture> Stream
    for RereadOnTrigger<TriggerStream, Failure, State, RecheckFn, ReadFn, ReadFuture>
where
    TriggerStream: Stream<Item = Result<(), TriggerFailure>>,
    TriggerFailure: Into<Failure>,
    RecheckFn: Fn(&Result<Sample, Failure>) -> RereadSchedule,
    ReadFn: FnMut(State) -> ReadFuture,
    ReadFuture: Future<Output = CarriedRead<State, Result<Sample, Failure>>>,
{
    type Item = Result<Sample, Failure>;

    fn poll_next(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Sample, Failure>>> {
        let mut projected = self.project();
        while let Poll::Ready(Some(trigger)) = projected.triggers.as_mut().poll_next(context) {
            match trigger {
                Ok(()) => *projected.triggered = true,
                Err(trigger_failure) => {
                    projected.trigger_failures.push_back(trigger_failure.into());
                }
            }
        }
        if let Some(trigger_failure) = projected.trigger_failures.pop_front() {
            return Poll::Ready(Some(Err(trigger_failure)));
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
        if projected.sample.is_none() {
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
            let carried_state = projected
                .carried_state
                .take()
                .expect("the carried state is back whenever no read is in flight");
            projected.sample.set(Some((projected.read)(carried_state)));
        }
        let Some(sample) = projected.sample.as_mut().as_pin_mut() else {
            return Poll::Pending;
        };
        let CarriedRead {
            carried_state,
            snapshot_fetch: produced,
        } = ready_poll!(sample.poll(context));
        projected.sample.set(None);
        *projected.carried_state = Some(carried_state);
        let next_deadline = match (projected.recheck_after)(&produced) {
            RereadSchedule::WaitForTrigger => None,
            RereadSchedule::RecheckAt(recheck_instant) => Some(sleep_until(recheck_instant.into())),
        };
        projected.deadline.set(next_deadline);
        Poll::Ready(Some(produced))
    }
}

struct LastDeliveredSnapshot<Source> {
    snapshot: Source,
    delivery_cadence: DeliveryCadence,
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
            |last_delivered: &mut Option<LastDeliveredSnapshot<Source>>, delivery| {
                let SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(snapshot),
                    delivery_cadence,
                } = &delivery
                else {
                    *last_delivered = None;
                    return ready(Some(delivery));
                };
                let repeated_snapshot = last_delivered.as_ref().is_some_and(|last| {
                    last.snapshot == *snapshot && last.delivery_cadence == *delivery_cadence
                });
                *last_delivered = Some(LastDeliveredSnapshot {
                    snapshot: snapshot.clone(),
                    delivery_cadence: *delivery_cadence,
                });
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeriodicPolling {
    pub poll_period: Duration,
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

pub fn triggered_deliveries_with_recheck<Source, State, TriggerFailure, ReadFuture>(
    triggers: impl Stream<Item = Result<(), TriggerFailure>> + Send + 'static,
    minimum_trigger_gap: Duration,
    recheck: PeriodicPolling,
    carried_state: State,
    read: impl FnMut(State) -> ReadFuture + Send + 'static,
    sampling_need: fn(&Result<Source, Source::Error>) -> SamplingNeed,
) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
where
    Source: SnapshotSource,
    State: Send + 'static,
    TriggerFailure: Into<Source::Error>,
    ReadFuture: Future<Output = CarriedRead<State, Result<Source, Source::Error>>> + Send + 'static,
{
    let delivery_cadence_after =
        move |snapshot_fetch: &Result<Source, Source::Error>| match sampling_need(snapshot_fetch) {
            SamplingNeed::KeepSampling => DeliveryCadence::LivePushWithRecheck(recheck),
            SamplingNeed::Settled => DeliveryCadence::LivePush,
        };
    triggers
        .paced_rereads_carrying(
            minimum_trigger_gap,
            carried_state,
            read,
            move |snapshot_fetch| match delivery_cadence_after(snapshot_fetch) {
                DeliveryCadence::LivePushWithRecheck(polling) => RereadSchedule::RecheckAt(
                    TokioClockInstant::now().into_std() + polling.poll_period,
                ),
                DeliveryCadence::PeriodicPolling(_)
                | DeliveryCadence::LivePush
                | DeliveryCadence::SingleRead => RereadSchedule::WaitForTrigger,
            },
        )
        .map(move |snapshot_fetch| SnapshotDelivery::SnapshotDelivered {
            delivery_cadence: delivery_cadence_after(&snapshot_fetch),
            snapshot_fetch,
        })
}
