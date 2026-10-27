pub mod ai_usage_spend;
pub mod battery_power_profiles;
pub mod blocking_background_work;
pub mod caldav_tasks_events;
pub mod cpu_memory_compute;
pub mod desktop_notifications;
pub mod desktop_windows_apps;
pub mod disk_storage;
pub mod gamepad_controller_input;
pub mod host_machine_identity;
pub mod login_session_presence;
pub mod media_playback_audio;
pub mod network_connectivity;
pub mod non_empty_text;
pub mod screen_brightness_color;
pub mod secret_file_reads;
pub mod sqlite_state_store;
pub mod stream_operators;
pub mod system_protocol_clients;
pub mod systemd_units_journal;
pub mod voice_dictation_input;
pub mod wallpaper_place_article;
pub mod weather;

use std::{
    fmt::{Debug, Display},
    future::{Future, ready},
    hash::Hash,
    time::Duration,
};

use futures::{Stream, StreamExt, stream};

use crate::stream_operators::{StreamOperators, connected_until_failure, ticked};

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

    fn delivery_cadence(&self) -> DeliveryCadence {
        Self::DELIVERY_CADENCE
    }
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
        .repeats_as_unchanged()
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
