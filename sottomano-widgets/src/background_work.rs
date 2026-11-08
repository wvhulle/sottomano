use std::{convert::identity, fmt::Display};

use iced::{Subscription, Task, futures::StreamExt};
use sottomano_desktop_sinks::{
    compositor_window_requests::compositor_requests::{
        self, CompositorRequest, CompositorRequestFailure,
    },
    desktop_windows_apps::clipboard_copy,
};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource, blocking_background_work::blocking,
};

pub fn compositor_request<Routed: From<CompositorRequestFailure> + Send + 'static>(
    request: CompositorRequest,
) -> Task<Routed> {
    Task::future(async move {
        compositor_requests::send_compositor_request(request)
            .await
            .err()
    })
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
        |subscribed| S::delivery_stream(subscribed.inputs.clone()),
    )
}

pub fn first_delivery<S: SnapshotSource>(inputs: S::StreamInputs) -> Task<SnapshotDelivery<S>> {
    Task::run(S::delivery_stream(inputs).take(1), identity)
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
    Task::future(blocking("clipboard", move || clipboard_copy::write(&text))).discard()
}
