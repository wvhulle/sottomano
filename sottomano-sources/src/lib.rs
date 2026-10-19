pub mod ai;
pub mod app_icon;
pub mod audio;
pub mod battery_power_profiles;
pub mod blocking_background_work;
pub mod coredumps;
pub mod cpu_memory_compute;
pub mod desktop_entries;
pub mod disk_storage;
pub mod favicons;
pub mod focus_ledger;
pub mod host_machine_identity;
pub mod icon_trim;
pub mod idle_notify;
pub mod job_view;
pub mod journal;
pub mod kernel_errors;
pub mod kitty_windows;
pub mod launch_tracker;
pub mod launcher_entry;
pub mod llama_server;
pub mod logind;
pub mod money;
pub mod mpris;
pub mod network_connectivity;
pub mod niri;
pub mod niri_config;
pub mod nix_package_facts;
pub mod nix_store;
pub mod non_empty_text;
pub mod notifications;
pub mod oom_kills;
pub mod previous_shutdown;
pub mod process;
pub mod project_directory;
pub mod screen_brightness_color;
pub mod secret_file_reads;
pub mod service_errors;
pub mod session_action;
pub mod sink;
pub mod sqlite_state_store;
pub mod stream_operators;
pub mod system_protocol_clients;
pub mod systemd_manager;
pub mod tray;
pub mod wallpaper;
pub mod weather;
pub mod window_label;

use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    future::{Future, ready},
    hash::Hash,
    time::Duration,
};

use futures::{Stream, StreamExt, stream};
use tokio::task::spawn_blocking;

use crate::stream_operators::{StreamOperators, connected_until_failure, ticked};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panicked;

impl Display for Panicked {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("reading task panicked")
    }
}

pub async fn blocking_poll<T, E>(
    read: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<Panicked> + Send + 'static,
{
    spawn_blocking(read)
        .await
        .unwrap_or_else(|_| Err(E::from(Panicked)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryCadence {
    PolledEvery(Duration),
    PushedLive,
    ReadOnce,
}

pub trait ReadingSource: Clone + Debug + Sized + Send + 'static {
    type Error: Clone + Debug + Display + Send + 'static;
    type StreamInputs: Clone + Hash + Send + Sync + 'static;

    const NAME: &'static str;
    const DELIVERY_CADENCE: DeliveryCadence;

    fn delivery_stream(key: Self::StreamInputs) -> impl Stream<Item = SourceDelivery<Self>> + Send;
}

#[derive(Debug, Clone)]
pub enum SourceDelivery<S: ReadingSource> {
    Absent,
    Reading(Result<S, S::Error>),
    Unchanged,
}

pub fn polled_delivery_stream<S, I, F>(
    inputs: I,
    poll: impl Fn(I) -> F + Send + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource + PartialEq,
    I: Clone + Send + 'static,
    F: Future<Output = Result<S, S::Error>> + Send,
{
    let DeliveryCadence::PolledEvery(every) = S::DELIVERY_CADENCE else {
        return stream::once(poll(inputs))
            .map(SourceDelivery::Reading)
            .left_stream();
    };
    ticked(every, move || poll(inputs.clone()))
        .map(SourceDelivery::Reading)
        .scan(
            None,
            |previous: &mut Option<SourceDelivery<S>>, delivery| {
                let repeated = previous.as_ref() == Some(&delivery);
                *previous = Some(delivery.clone());
                ready(Some(if repeated {
                    SourceDelivery::Unchanged
                } else {
                    delivery
                }))
            },
        )
        .right_stream()
}

pub fn polled_delivery_after_setup<S, P, F>(
    setup: impl Future<Output = Result<P, S::Error>> + Send,
    poll: impl Fn(P) -> F + Send + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource + PartialEq,
    P: Clone + Send + 'static,
    F: Future<Output = Result<S, S::Error>> + Send,
{
    stream::once(setup).flat_map(move |prepared| match prepared {
        Ok(prepared) => polled_delivery_stream(prepared, poll.clone()).left_stream(),
        Err(failure) => stream::once(ready(SourceDelivery::Reading(Err(failure)))).right_stream(),
    })
}

pub fn pushed_delivery_stream<S, R>(
    connect: impl Future<Output = Result<R, S::Error>> + Send,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource + PartialEq,
    R: Stream<Item = Result<S, S::Error>> + Send,
{
    connected_until_failure(connect)
        .map(SourceDelivery::Reading)
        .distinct_until_changed()
}

impl<S: ReadingSource + PartialEq> PartialEq for SourceDelivery<S> {
    fn eq(&self, other: &Self) -> bool {
        match self {
            SourceDelivery::Absent => matches!(other, SourceDelivery::Absent),
            SourceDelivery::Reading(Ok(value)) => {
                matches!(other, SourceDelivery::Reading(Ok(known)) if known == value)
            }
            SourceDelivery::Reading(Err(_)) => false,
            SourceDelivery::Unchanged => matches!(other, SourceDelivery::Unchanged),
        }
    }
}
