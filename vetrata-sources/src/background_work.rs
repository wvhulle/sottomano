use std::{convert::identity, fmt::Display, future::Future};

use iced::{
    Subscription, Task,
    futures::{Stream, StreamExt},
};
use tokio::task::spawn_blocking;

use crate::{ReadingSource, SourceDelivery, sink::clipboard, stream_operators::restarting};

pub fn blocking<T: Default + Send + 'static>(
    context: &'static str,
    work: impl FnOnce() -> T + Send + 'static,
) -> Task<T> {
    Task::perform(
        async move {
            let span = tracing::debug_span!("blocking", context);
            spawn_blocking(move || span.in_scope(work))
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("{context}: task panicked: {e}");
                    T::default()
                })
        },
        identity,
    )
}

pub fn restarting_subscription<F, S, E>(
    source: &'static str,
    connect: fn() -> F,
) -> Subscription<S::Item>
where
    F: Future<Output = Result<S, E>> + Send + 'static,
    S: Stream + Send + 'static,
    S::Item: Send + 'static,
    E: Display + Send + 'static,
{
    Subscription::run_with((source, connect), |&(source, connect)| {
        restarting(source, connect)
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubscribedSource<I> {
    name: &'static str,
    inputs: I,
}

pub fn source_subscription<S: ReadingSource>(
    inputs: S::StreamInputs,
) -> Subscription<SourceDelivery<S>> {
    Subscription::run_with(
        SubscribedSource {
            name: S::NAME,
            inputs,
        },
        |subscribed| S::delivery_stream(subscribed.inputs.clone()),
    )
}

pub fn first_delivery<S: ReadingSource>(inputs: S::StreamInputs) -> Task<SourceDelivery<S>> {
    Task::run(S::delivery_stream(inputs).take(1), identity)
}

pub fn copy_to_clipboard<M: Send + 'static>(text: String) -> Task<M> {
    blocking("clipboard", move || clipboard::write(&text)).discard()
}
