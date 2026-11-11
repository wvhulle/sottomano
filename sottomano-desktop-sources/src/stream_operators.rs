use std::{
    future::{Future, ready},
    pin::Pin,
    task::{Context, Poll, ready},
    time::{Duration, Instant},
};

use futures::{
    Stream, StreamExt, TryStreamExt,
    stream::{self, Fuse, FusedStream},
};
use pin_project_lite::pin_project;
use tokio::time::{Sleep, sleep, sleep_until};

use crate::{SnapshotDelivery, SnapshotSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RereadSchedule {
    WaitForTrigger,
    RecheckAt(Instant),
}

pub fn read_once_then_per_burst<Produced, ReadFuture>(
    triggers: impl Stream<Item = ()> + Send + 'static,
    read: impl FnMut() -> ReadFuture + Send + 'static,
) -> impl Stream<Item = Produced> + Send
where
    Produced: Send + 'static,
    ReadFuture: Future<Output = Produced> + Send,
{
    reread_on_trigger_or_deadline(triggers, read, |_| RereadSchedule::WaitForTrigger)
}

pub fn reread_on_trigger_or_deadline<Produced, ReadFuture>(
    triggers: impl Stream<Item = ()> + Send + 'static,
    read: impl FnMut() -> ReadFuture + Send + 'static,
    schedule_after: impl Fn(&Produced) -> RereadSchedule + Send + 'static,
) -> impl Stream<Item = Produced> + Send
where
    Produced: Send + 'static,
    ReadFuture: Future<Output = Produced> + Send,
{
    reread_on_spaced_trigger_or_deadline(triggers, Duration::ZERO, read, schedule_after)
}

pub fn reread_on_spaced_trigger_or_deadline<Produced, ReadFuture>(
    triggers: impl Stream<Item = ()> + Send + 'static,
    minimum_trigger_gap: Duration,
    read: impl FnMut() -> ReadFuture + Send + 'static,
    schedule_after: impl Fn(&Produced) -> RereadSchedule + Send + 'static,
) -> impl Stream<Item = Produced> + Send
where
    Produced: Send + 'static,
    ReadFuture: Future<Output = Produced> + Send,
{
    RereadOnTrigger {
        triggers: triggers.fuse(),
        minimum_trigger_gap,
        read,
        schedule_after,
        reading: None,
        cooldown: None,
        deadline: None,
        triggered: true,
    }
}

pin_project! {
    struct RereadOnTrigger<TriggerStream, ReadFn, ScheduleFn, ReadFuture> {
        #[pin]
        triggers: Fuse<TriggerStream>,
        minimum_trigger_gap: Duration,
        read: ReadFn,
        schedule_after: ScheduleFn,
        #[pin]
        reading: Option<ReadFuture>,
        #[pin]
        cooldown: Option<Sleep>,
        #[pin]
        deadline: Option<Sleep>,
        triggered: bool,
    }
}

impl<TriggerStream, ReadFn, ScheduleFn, ReadFuture> Stream
    for RereadOnTrigger<TriggerStream, ReadFn, ScheduleFn, ReadFuture>
where
    TriggerStream: Stream<Item = ()>,
    ReadFn: FnMut() -> ReadFuture,
    ScheduleFn: Fn(&ReadFuture::Output) -> RereadSchedule,
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
        let produced = ready!(reading.poll(context));
        projected.reading.set(None);
        let next_deadline = match (projected.schedule_after)(&produced) {
            RereadSchedule::WaitForTrigger => None,
            RereadSchedule::RecheckAt(recheck_instant) => Some(sleep_until(recheck_instant.into())),
        };
        projected.deadline.set(next_deadline);
        Poll::Ready(Some(produced))
    }
}

pub fn connected_until_failure<ConnectedStream, ItemValue, Failure>(
    connecting: impl Future<Output = Result<ConnectedStream, Failure>> + Send,
) -> impl Stream<Item = Result<ItemValue, Failure>> + Send
where
    ConnectedStream: Stream<Item = Result<ItemValue, Failure>> + Send,
    ItemValue: Send,
    Failure: Send,
{
    until_first_failure(stream::once(connecting).try_flatten())
}

enum SubscriptionPhase<ConnectedStream> {
    Connecting,
    Streaming(ConnectedStream),
    Reconnecting,
}

pub fn resubscribed<ConnectedStream, SubscribeFuture, ItemValue, Failure>(
    retry_gap: Duration,
    subscribe: impl FnMut() -> SubscribeFuture + Send,
) -> impl Stream<Item = Result<ItemValue, Failure>> + Send
where
    ConnectedStream: Stream<Item = Result<ItemValue, Failure>> + Send,
    SubscribeFuture: Future<Output = Result<ConnectedStream, Failure>> + Send,
    ItemValue: Send,
    Failure: Send,
{
    stream::unfold(
        (SubscriptionPhase::Connecting, subscribe),
        move |(phase, mut subscribe)| {
            let retry_gap = retry_gap;
            async move {
                let mut phase = phase;
                loop {
                    match phase {
                        SubscriptionPhase::Connecting | SubscriptionPhase::Reconnecting => {
                            if matches!(phase, SubscriptionPhase::Reconnecting) {
                                sleep(retry_gap).await;
                            }
                            phase = match subscribe().await {
                                Ok(connected) => SubscriptionPhase::Streaming(Box::pin(connected)),
                                Err(failure) => {
                                    return Some((
                                        Err(failure),
                                        (SubscriptionPhase::Reconnecting, subscribe),
                                    ));
                                }
                            };
                        }
                        SubscriptionPhase::Streaming(mut connected) => {
                            match connected.as_mut().next().await {
                                Some(delivered) => {
                                    return Some((
                                        delivered,
                                        (SubscriptionPhase::Streaming(connected), subscribe),
                                    ));
                                }
                                None => phase = SubscriptionPhase::Reconnecting,
                            }
                        }
                    }
                }
            }
        },
    )
}

fn until_first_failure<ItemValue, Failure>(
    outcomes: impl Stream<Item = Result<ItemValue, Failure>> + Send,
) -> impl Stream<Item = Result<ItemValue, Failure>> + Send
where
    ItemValue: Send,
    Failure: Send,
{
    let mut failed = false;
    outcomes.take_while(move |outcome| {
        let before_failure = !failed;
        failed = outcome.is_err();
        ready(before_failure)
    })
}

pub trait StreamOperators: Stream + Sized {
    fn distinct_until_changed(self) -> impl Stream<Item = Self::Item> + Send
    where
        Self: Send,
        Self::Item: PartialEq + Clone + Send,
    {
        self.scan(None, |previous: &mut Option<Self::Item>, item| {
            let repeated = previous.as_ref() == Some(&item);
            *previous = Some(item.clone());
            ready(Some((!repeated).then_some(item)))
        })
        .filter_map(ready)
    }

    fn repeats_as_unchanged<Source>(self) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Self: Stream<Item = SnapshotDelivery<Source>> + Send,
        Source: SnapshotSource + PartialEq,
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

    fn keeping_alive<Guard: Send>(self, guard: Guard) -> impl Stream<Item = Self::Item> + Send
    where
        Self: Send,
    {
        self.map(move |item| {
            let _ = &guard;
            item
        })
    }

    fn switch_map<SwitchFn, InnerStream>(
        self,
        switch: SwitchFn,
    ) -> SwitchMap<Self, SwitchFn, InnerStream>
    where
        SwitchFn: FnMut(Self::Item) -> InnerStream,
        InnerStream: Stream,
    {
        SwitchMap {
            outer: self.fuse(),
            switch,
            inner: None,
        }
    }

    fn available_while<Source, InnerStream>(
        self,
        start: impl FnMut() -> InnerStream + Send,
    ) -> impl Stream<Item = SnapshotDelivery<Source>> + Send
    where
        Self: Stream<Item = Availability> + Send,
        Source: SnapshotSource,
        InnerStream: Stream<Item = SnapshotDelivery<Source>> + Send,
    {
        self.while_held(start, SnapshotDelivery::SourceUnavailable)
    }

    fn while_held<InnerStream>(
        self,
        mut start: impl FnMut() -> InnerStream + Send,
        released: InnerStream::Item,
    ) -> impl Stream<Item = InnerStream::Item> + Send
    where
        Self: Stream<Item = Availability> + Send,
        InnerStream: Stream + Send,
        InnerStream::Item: Clone + Send,
    {
        self.distinct_until_changed()
            .switch_map(move |availability| match availability {
                Availability::Available => start().left_stream(),
                Availability::Unavailable => stream::iter([released.clone()]).right_stream(),
            })
    }
}

impl<SourceStream: Stream> StreamOperators for SourceStream {}

pin_project! {
    pub struct SwitchMap<OuterStream, SwitchFn, InnerStream> {
        #[pin]
        outer: Fuse<OuterStream>,
        switch: SwitchFn,
        #[pin]
        inner: Option<InnerStream>,
    }
}

impl<OuterStream, SwitchFn, InnerStream> Stream for SwitchMap<OuterStream, SwitchFn, InnerStream>
where
    OuterStream: Stream,
    SwitchFn: FnMut(OuterStream::Item) -> InnerStream,
    InnerStream: Stream,
{
    type Item = InnerStream::Item;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut projected = self.project();
        while let Poll::Ready(Some(item)) = projected.outer.as_mut().poll_next(context) {
            projected.inner.set(Some((projected.switch)(item)));
        }
        let outer_ended = projected.outer.is_terminated();
        let idle = if outer_ended {
            Poll::Ready(None)
        } else {
            Poll::Pending
        };
        let Some(inner) = projected.inner.as_mut().as_pin_mut() else {
            return idle;
        };
        match inner.poll_next(context) {
            Poll::Ready(None) => {
                projected.inner.set(None);
                idle
            }
            polled => polled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable,
}

#[cfg(test)]
mod tests {
    use std::{pin::pin, vec::IntoIter};

    use futures::channel::mpsc;
    use tokio::time::{self, timeout};

    use super::*;

    const MINIMUM_GAP: Duration = Duration::from_secs(60);

    #[tokio::test(start_paused = true)]
    async fn a_resubscribed_stream_reconnects_after_its_source_ends() {
        let started = time::Instant::now();
        let mut planned_attempts = std::vec![
            stream::iter(vec![Ok(1)]),
            stream::iter(vec![Ok(0), Err("source ended")]),
        ];
        let mut resubscribed_events = pin!(resubscribed(MINIMUM_GAP, move || {
            let next_attempt = planned_attempts
                .pop()
                .unwrap_or_else(|| stream::iter(Vec::new()));
            async move { Ok(next_attempt) }
        }));
        assert_eq!(resubscribed_events.next().await, Some(Ok(0)));
        assert_eq!(resubscribed_events.next().await, Some(Err("source ended")));
        assert_eq!(started.elapsed(), Duration::ZERO);
        assert_eq!(resubscribed_events.next().await, Some(Ok(1)));
        assert_eq!(started.elapsed(), MINIMUM_GAP);
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_subscription_retries_after_the_gap() {
        let started = time::Instant::now();
        let mut resubscribed_events = pin!(resubscribed::<
            stream::Iter<IntoIter<Result<u8, &'static str>>>,
            _,
            u8,
            &'static str,
        >(MINIMUM_GAP, || async { Err("refused") }));
        assert_eq!(resubscribed_events.next().await, Some(Err("refused")));
        assert_eq!(started.elapsed(), Duration::ZERO);
        assert_eq!(resubscribed_events.next().await, Some(Err("refused")));
        assert_eq!(started.elapsed(), MINIMUM_GAP);
    }

    #[tokio::test(start_paused = true)]
    async fn spaced_rereads_drain_burst_triggers_and_wait_the_gap() {
        let started = time::Instant::now();
        let (trigger_sender, trigger_receiver) = mpsc::unbounded();
        let mut read_count = 0_u32;
        let mut rereads = pin!(reread_on_spaced_trigger_or_deadline(
            trigger_receiver,
            MINIMUM_GAP,
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
        assert_eq!(started.elapsed(), MINIMUM_GAP);
        assert!(timeout(MINIMUM_GAP * 10, rereads.next()).await.is_err());
    }
}
