use std::{
    convert::identity,
    fmt::Display,
    future::{Future, ready},
    pin::Pin,
    task::{Context, Poll, ready},
    time::{Duration, Instant},
};

use futures::{
    FutureExt, Stream, StreamExt, TryStreamExt,
    stream::{self, Fuse, FusedStream},
};
use pin_project_lite::pin_project;
use tokio::time::{Sleep, sleep, sleep_until};

use crate::{SnapshotDelivery, SnapshotSource};

pub fn read_once_then_per_burst<T, F>(
    triggers: impl Stream<Item = ()> + Send + 'static,
    read: impl FnMut() -> F + Send + 'static,
) -> impl Stream<Item = T> + Send
where
    T: Send + 'static,
    F: Future<Output = T> + Send,
{
    reread_on_trigger_or_deadline(triggers, read, |_| None)
}

pub fn reread_on_trigger_or_deadline<T, F>(
    triggers: impl Stream<Item = ()> + Send + 'static,
    read: impl FnMut() -> F + Send + 'static,
    deadline_after: impl Fn(&T) -> Option<Instant> + Send + 'static,
) -> impl Stream<Item = T> + Send
where
    T: Send + 'static,
    F: Future<Output = T> + Send,
{
    RereadOnTrigger {
        triggers: triggers.fuse(),
        read,
        deadline_after,
        reading: None,
        deadline: None,
        triggered: true,
    }
}

pin_project! {
    struct RereadOnTrigger<S, Read, Deadline, F> {
        #[pin]
        triggers: Fuse<S>,
        read: Read,
        deadline_after: Deadline,
        #[pin]
        reading: Option<F>,
        #[pin]
        deadline: Option<Sleep>,
        triggered: bool,
    }
}

impl<S, Read, Deadline, F> Stream for RereadOnTrigger<S, Read, Deadline, F>
where
    S: Stream<Item = ()>,
    Read: FnMut() -> F,
    Deadline: Fn(&F::Output) -> Option<Instant>,
    F: Future,
{
    type Item = F::Output;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<F::Output>> {
        let mut this = self.project();
        while let Poll::Ready(Some(())) = this.triggers.as_mut().poll_next(cx) {
            *this.triggered = true;
        }
        if let Some(deadline) = this.deadline.as_mut().as_pin_mut()
            && deadline.poll(cx).is_ready()
        {
            *this.triggered = true;
            this.deadline.set(None);
        }
        if this.reading.is_none() {
            if !*this.triggered {
                return if this.triggers.is_terminated() {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                };
            }
            *this.triggered = false;
            this.reading.set(Some((this.read)()));
        }
        let Some(reading) = this.reading.as_mut().as_pin_mut() else {
            return Poll::Pending;
        };
        let produced = ready!(reading.poll(cx));
        this.reading.set(None);
        let next_deadline = (this.deadline_after)(&produced).map(|at| sleep_until(at.into()));
        this.deadline.set(next_deadline);
        Poll::Ready(Some(produced))
    }
}

pub fn connected_until_failure<S, T, E>(
    connecting: impl Future<Output = Result<S, E>> + Send,
) -> impl Stream<Item = Result<T, E>> + Send
where
    S: Stream<Item = Result<T, E>> + Send,
    T: Send,
    E: Send,
{
    until_first_failure(stream::once(connecting).try_flatten())
}

pub fn resubscribing_until_failure<F, S, T, E>(
    subscribe: impl FnMut() -> F + Send,
) -> impl Stream<Item = Result<T, E>> + Send
where
    F: Future<Output = Result<S, E>> + Send,
    S: Stream<Item = Result<T, E>> + Send,
    T: Send,
    E: Send,
{
    until_first_failure(stream::repeat_with(subscribe).then(identity).try_flatten())
}

fn until_first_failure<T, E>(
    results: impl Stream<Item = Result<T, E>> + Send,
) -> impl Stream<Item = Result<T, E>> + Send
where
    T: Send,
    E: Send,
{
    let mut failed = false;
    results.take_while(move |outcome| {
        let before_failure = !failed;
        failed = outcome.is_err();
        ready(before_failure)
    })
}

pub fn restarting<F, S, E>(
    source: &'static str,
    connect: impl FnMut() -> F,
) -> impl Stream<Item = S::Item>
where
    F: Future<Output = Result<S, E>>,
    S: Stream,
    E: Display,
{
    stream::repeat_with(connect)
        .then(identity)
        .inspect_err(move |e| tracing::warn!("{source}: {e}"))
        .take_while(|attempt| ready(attempt.is_ok()))
        .filter_map(|attempt| ready(attempt.ok()))
        .flatten()
}

pub trait StreamOperators: Stream + Sized {
    fn at_most_once_per(self, minimum_gap: Duration) -> impl Stream<Item = ()> + Send
    where
        Self: Stream<Item = ()> + Send,
    {
        stream::unfold(Box::pin(self.fuse()), move |mut triggers| async move {
            triggers.next().await?;
            sleep(minimum_gap).await;
            while let Some(Some(())) = triggers.next().now_or_never() {}
            Some(((), triggers))
        })
    }

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

    fn repeats_as_unchanged<S>(self) -> impl Stream<Item = SnapshotDelivery<S>> + Send
    where
        Self: Stream<Item = SnapshotDelivery<S>> + Send,
        S: SnapshotSource + PartialEq,
    {
        self.scan(
            None,
            |previous: &mut Option<SnapshotDelivery<S>>, delivery| {
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

    fn switch_map<F, R>(self, switch: F) -> SwitchMap<Self, F, R>
    where
        F: FnMut(Self::Item) -> R,
        R: Stream,
    {
        SwitchMap {
            outer: self.fuse(),
            switch,
            inner: None,
        }
    }

    fn available_while<S, R>(
        self,
        start: impl FnMut() -> R + Send,
    ) -> impl Stream<Item = SnapshotDelivery<S>> + Send
    where
        Self: Stream<Item = bool> + Send,
        S: SnapshotSource,
        R: Stream<Item = SnapshotDelivery<S>> + Send,
    {
        self.while_held(start, SnapshotDelivery::SourceUnavailable)
    }

    fn while_held<R>(
        self,
        mut start: impl FnMut() -> R + Send,
        released: R::Item,
    ) -> impl Stream<Item = R::Item> + Send
    where
        Self: Stream<Item = bool> + Send,
        R: Stream + Send,
        R::Item: Clone + Send,
    {
        self.distinct_until_changed().switch_map(move |held| {
            if held {
                start().left_stream()
            } else {
                stream::iter([released.clone()]).right_stream()
            }
        })
    }
}

impl<S: Stream> StreamOperators for S {}

pin_project! {
    pub struct SwitchMap<O, F, R> {
        #[pin]
        outer: Fuse<O>,
        switch: F,
        #[pin]
        inner: Option<R>,
    }
}

impl<O, F, R> Stream for SwitchMap<O, F, R>
where
    O: Stream,
    F: FnMut(O::Item) -> R,
    R: Stream,
{
    type Item = R::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        while let Poll::Ready(Some(item)) = this.outer.as_mut().poll_next(cx) {
            this.inner.set(Some((this.switch)(item)));
        }
        let outer_ended = this.outer.is_terminated();
        let idle = if outer_ended {
            Poll::Ready(None)
        } else {
            Poll::Pending
        };
        let Some(inner) = this.inner.as_mut().as_pin_mut() else {
            return idle;
        };
        match inner.poll_next(cx) {
            Poll::Ready(None) => {
                this.inner.set(None);
                idle
            }
            polled => polled,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::pin::pin;

    use futures::channel::mpsc;
    use tokio::time::{self, timeout};

    use super::*;

    const MINIMUM_GAP: Duration = Duration::from_secs(60);

    #[tokio::test(start_paused = true)]
    async fn a_burst_of_triggers_yields_once_after_the_gap() {
        let started = time::Instant::now();
        let burst = stream::iter([(), (), ()]).chain(stream::pending());
        let mut limited = pin!(burst.at_most_once_per(MINIMUM_GAP));
        assert_eq!(limited.next().await, Some(()));
        assert_eq!(started.elapsed(), MINIMUM_GAP);
        assert!(timeout(MINIMUM_GAP * 10, limited.next()).await.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn triggers_arriving_after_a_yield_wait_a_full_gap() {
        let started = time::Instant::now();
        let (trigger_sender, trigger_receiver) = mpsc::unbounded();
        let mut limited = pin!(trigger_receiver.at_most_once_per(MINIMUM_GAP));
        trigger_sender.unbounded_send(()).unwrap();
        assert_eq!(limited.next().await, Some(()));
        trigger_sender.unbounded_send(()).unwrap();
        assert_eq!(limited.next().await, Some(()));
        assert_eq!(started.elapsed(), MINIMUM_GAP * 2);
    }

    #[tokio::test(start_paused = true)]
    async fn silence_yields_nothing() {
        let mut limited = pin!(stream::pending::<()>().at_most_once_per(MINIMUM_GAP));
        assert!(timeout(MINIMUM_GAP * 10, limited.next()).await.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn an_ended_trigger_stream_ends_the_limited_stream() {
        let mut limited = pin!(stream::empty::<()>().at_most_once_per(MINIMUM_GAP));
        assert_eq!(limited.next().await, None);
    }
}
