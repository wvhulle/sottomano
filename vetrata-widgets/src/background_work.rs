use std::{convert::identity, fmt::Display, future::Future};

use iced::{
    Subscription, Task,
    futures::{Stream, StreamExt},
};
use vetrata_sources::{
    ReadingSource, SourceDelivery, background_work::blocking, sink::clipboard,
    stream_operators::restarting,
};

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
    Task::future(blocking("clipboard", move || clipboard::write(&text))).discard()
}
