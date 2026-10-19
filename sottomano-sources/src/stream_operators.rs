use std::{
    convert::identity,
    fmt::Display,
    future::{Future, ready},
    pin::Pin,
    task::{Context, Poll, ready},
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use futures::{
    Stream, StreamExt, TryStreamExt,
    stream::{self, Fuse, FusedStream},
};
use pin_project_lite::pin_project;
use tokio::time::{MissedTickBehavior, Sleep, interval, sleep, sleep_until};
use tokio_stream::wrappers::IntervalStream;

use crate::{ReadingSource, SourceDelivery};

pub fn ticked<T, F>(
    every: Duration,
    work: impl Fn() -> F + Send + Clone + 'static,
) -> impl Stream<Item = T>
where
    T: Send + 'static,
    F: Future<Output = T> + Send,
{
    let mut timer = interval(every);
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    IntervalStream::new(timer).then(move |_| work())
}

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
    stream::once(connecting).try_flatten().until_first_failure()
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
    stream::repeat_with(subscribe)
        .then(identity)
        .try_flatten()
        .until_first_failure()
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

    fn until_first_failure<T, E>(self) -> impl Stream<Item = Result<T, E>> + Send
    where
        Self: Stream<Item = Result<T, E>> + Send,
        T: Send,
        E: Send,
    {
        let mut failed = false;
        self.take_while(move |item| {
            let before_failure = !failed;
            failed = item.is_err();
            ready(before_failure)
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

pub fn ticked_at_boundaries<S: ReadingSource>(
    period: Duration,
    reading: impl Fn() -> S + Send + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send {
    stream::repeat_with(move || {
        let reading = reading.clone();
        async move {
            sleep(until_next_boundary(Local::now(), period)).await;
            reading()
        }
    })
    .then(identity)
    .map(|reading| SourceDelivery::Reading(Ok(reading)))
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
mod boundary_tests {
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
