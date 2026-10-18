use std::{
    convert::identity,
    fmt::Display,
    future::{Future, ready},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use iced::futures::{
    Stream, StreamExt, TryStreamExt,
    stream::{self, Fuse, FusedStream},
};
use pin_project_lite::pin_project;
use tokio::time::{MissedTickBehavior, interval};
use tokio_stream::wrappers::IntervalStream;

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
