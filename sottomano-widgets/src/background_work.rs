use std::{convert::identity, fmt::Display, task::Poll};

use iced::{
    Subscription, Task,
    futures::{Stream, StreamExt, stream},
};
use sottomano_compositor_model::compositor_requests::{
    CompositorRequest, CompositorRequestFailure,
};
use sottomano_compositors::compositor_connection::send_compositor_request;
use sottomano_desktop_sinks::desktop_windows_apps::clipboard_copy;
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, blocking_background_work::blocking,
};

use crate::load_tally::{PollOutcome, WorkMeasurement, record_source_poll};

pub fn compositor_request<Routed: From<CompositorRequestFailure> + Send + 'static>(
    request: CompositorRequest,
) -> Task<Routed> {
    Task::future(async move { send_compositor_request(request).await.err() })
        .and_then(|failure| Task::done(Routed::from(failure)))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SubscribedSource<I> {
    name: &'static str,
    inputs: I,
}

pub fn snapshot_subscription<S: SnapshotSource>(
    inputs: S::StreamInputs,
) -> Subscription<SnapshotDelivery<S>> {
    Subscription::run_with(
        SubscribedSource {
            name: S::NAME,
            inputs,
        },
        |subscribed| source_spanned_stream(S::NAME, S::delivery_stream(subscribed.inputs.clone())),
    )
}

fn source_spanned_stream<Delivery>(
    source_name: &'static str,
    deliveries: impl Stream<Item = Delivery> + Send + 'static,
) -> impl Stream<Item = Delivery> + Send + 'static {
    let source_span = tracing::debug_span!("snapshot_source", source = source_name);
    let mut pinned_deliveries = Box::pin(deliveries);
    stream::poll_fn(move |context| {
        let measurement = WorkMeasurement::start();
        let polled = source_span.in_scope(|| pinned_deliveries.as_mut().poll_next(context));
        let outcome = match &polled {
            Poll::Ready(Some(_)) => PollOutcome::Delivered,
            Poll::Ready(None) => PollOutcome::Finished,
            Poll::Pending => PollOutcome::Pending,
        };
        record_source_poll(source_name, measurement.finish(), outcome);
        polled
    })
}

pub fn first_delivery<S: SnapshotSource>(inputs: S::StreamInputs) -> Task<SnapshotDelivery<S>> {
    Task::run(
        source_spanned_stream(S::NAME, S::delivery_stream(inputs)).take(1),
        identity,
    )
}

pub fn logged_failure_task<M: Send + 'static, Failure: Display>(
    action: String,
    work: impl Future<Output = Result<(), Failure>> + Send + 'static,
) -> Task<M> {
    Task::future(async move {
        if let Err(failure) = work.await {
            tracing::warn!("{action}: {failure}");
        }
    })
    .discard()
}

pub fn copy_to_clipboard<M: Send + 'static>(text: String) -> Task<M> {
    Task::future(blocking("clipboard", move || {
        if let Err(copy_failure) = clipboard_copy::write(&text) {
            tracing::warn!("clipboard: copying {} bytes: {copy_failure}", text.len());
        }
    }))
    .discard()
}
