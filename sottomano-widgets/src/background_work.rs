use std::convert::identity;

use iced::{Subscription, Task, futures::StreamExt};
use sottomano_sources::{
    SnapshotDelivery, SnapshotSource,
    blocking_background_work::blocking,
    desktop_windows_apps::{clipboard_copy, niri_action_requests},
};

pub fn niri_action<Routed: From<niri_action_requests::ActionFailure> + Send + 'static>(
    action: niri_ipc::Action,
) -> Task<Routed> {
    Task::future(blocking("niri action", move || {
        niri_action_requests::send_action(&action).err()
    }))
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

pub fn copy_to_clipboard<M: Send + 'static>(text: String) -> Task<M> {
    Task::future(blocking("clipboard", move || clipboard_copy::write(&text))).discard()
}
