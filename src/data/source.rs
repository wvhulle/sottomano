use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    future::{Future, ready},
    hash::Hash,
    mem::take,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use iced::futures::{Stream, StreamExt, stream};
use pin_project_lite::pin_project;
use tokio::task::spawn_blocking;

use crate::{message_subscriptions::ticked, widgets::tooltip_body::Section};

pub mod ai;
pub mod app_icon;
pub mod audio;
pub mod backlight;
pub mod bus;
pub mod clight;
pub mod command;
pub mod coredumps;
pub mod desktop_entries;
pub mod disk;
pub mod favicons;
pub mod file_presence;
pub mod focus_ledger;
pub mod fwupd;
pub mod gamma;
pub mod gpu;
pub mod hardware;
pub mod hostname;
pub mod http;
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
pub mod mpris;
pub mod net_interfaces;
pub mod niri;
pub mod niri_config;
pub mod nix_package_facts;
pub mod nix_store;
pub mod oom_kills;
pub mod power_profile;
pub mod previous_shutdown;
pub mod process;
pub mod project_directory;
pub mod service_errors;
pub mod session_action;
pub mod sysfs_sensors;
pub mod systemd_manager;
pub mod tailscale;
pub mod throughput;
pub mod tray;
pub mod umbra;
pub mod wallpaper;
pub mod weather;
pub mod window_label;
pub mod wireguard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panicked;

impl Display for Panicked {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("reading task panicked")
    }
}

impl From<Panicked> for Section {
    fn from(panicked: Panicked) -> Self {
        Section::error(panicked)
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
}

pub trait ReadingSource: Clone + Debug + Sized + Send + 'static {
    type Error: Clone + Debug + Display + Send + Into<Section> + 'static;
    type StreamInputs: Clone + Hash + Send + Sync + 'static;

    const NAME: &'static str;
    const DELIVERY_CADENCE: DeliveryCadence;

    fn delivery_stream(key: Self::StreamInputs) -> impl Stream<Item = SourceDelivery<Self>> + Send;
}

#[derive(Debug, Clone)]
pub enum SourceDelivery<S: ReadingSource> {
    Absent,
    Reading(Result<S, S::Error>),
}

pub fn polled_delivery_stream<S, F>(
    inputs: S::StreamInputs,
    poll: impl Fn(S::StreamInputs) -> F + Send + Clone + 'static,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource + PartialEq,
    F: Future<Output = Result<S, S::Error>> + Send,
{
    let DeliveryCadence::PolledEvery(every) = S::DELIVERY_CADENCE else {
        return stream::once(poll(inputs))
            .map(SourceDelivery::Reading)
            .left_stream();
    };
    drop_unchanged_deliveries(
        ticked(every, move || poll(inputs.clone())).map(SourceDelivery::Reading),
    )
    .right_stream()
}

pub fn present_or_absent<S, R>(
    presence: impl Stream<Item = bool> + Send,
    readings: impl FnMut() -> R + Send,
) -> impl Stream<Item = SourceDelivery<S>> + Send
where
    S: ReadingSource,
    R: Stream<Item = SourceDelivery<S>> + Send,
{
    while_present(presence, readings).map(|delivery| delivery.unwrap_or(SourceDelivery::Absent))
}

pub fn while_present<P, F, R>(presence: P, readings: F) -> WhilePresent<P, F, R>
where
    P: Stream<Item = bool>,
    F: FnMut() -> R,
    R: Stream,
{
    WhilePresent {
        presence,
        presence_ended: false,
        start: readings,
        inner: None,
    }
}

pin_project! {
    pub struct WhilePresent<P, F, R> {
        #[pin]
        presence: P,
        presence_ended: bool,
        start: F,
        #[pin]
        inner: Option<R>,
    }
}

impl<P, F, R> Stream for WhilePresent<P, F, R>
where
    P: Stream<Item = bool>,
    F: FnMut() -> R,
    R: Stream,
{
    type Item = Option<R::Item>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        while !*this.presence_ended {
            match this.presence.as_mut().poll_next(cx) {
                Poll::Ready(Some(true)) => {
                    if this.inner.is_none() {
                        this.inner.set(Some((this.start)()));
                    }
                }
                Poll::Ready(Some(false)) => {
                    if this.inner.is_some() {
                        this.inner.set(None);
                        return Poll::Ready(Some(None));
                    }
                }
                Poll::Ready(None) => *this.presence_ended = true,
                Poll::Pending => break,
            }
        }
        let Some(inner) = this.inner.as_mut().as_pin_mut() else {
            return if *this.presence_ended {
                Poll::Ready(None)
            } else {
                Poll::Pending
            };
        };
        match inner.poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(Some(item))),
            Poll::Ready(None) => {
                this.inner.set(None);
                if *this.presence_ended {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[derive(Clone, Copy)]
enum ChangedCondition {
    First(bool),
    Second(bool),
}

#[derive(Default)]
struct LatestConditions {
    first: Option<bool>,
    second: Option<bool>,
}

impl LatestConditions {
    fn with(self, changed: ChangedCondition) -> Self {
        match changed {
            ChangedCondition::First(held) => Self {
                first: Some(held),
                ..self
            },
            ChangedCondition::Second(held) => Self {
                second: Some(held),
                ..self
            },
        }
    }

    fn both_held(&self) -> Option<bool> {
        Some(self.first? && self.second?)
    }
}

pub fn both_held(
    first: impl Stream<Item = bool> + Send,
    second: impl Stream<Item = bool> + Send,
) -> impl Stream<Item = bool> + Send {
    stream::select(
        first.map(ChangedCondition::First),
        second.map(ChangedCondition::Second),
    )
    .scan(LatestConditions::default(), |latest, changed| {
        *latest = take(latest).with(changed);
        ready(Some(latest.both_held()))
    })
    .filter_map(ready)
}

pub fn emit_changes<T: Clone + PartialEq>(
    readings: impl Stream<Item = T>,
) -> impl Stream<Item = T> {
    readings
        .scan(None, |previous, reading| {
            let changed = previous.as_ref() != Some(&reading);
            if changed {
                *previous = Some(reading.clone());
            }
            ready(Some(changed.then_some(reading)))
        })
        .filter_map(ready)
}

pub fn drop_unchanged_deliveries<S: ReadingSource + PartialEq>(
    deliveries: impl Stream<Item = SourceDelivery<S>> + Send,
) -> impl Stream<Item = SourceDelivery<S>> + Send {
    deliveries
        .scan(LastDelivery::Nothing, |last, delivery| {
            let emit = match &delivery {
                SourceDelivery::Absent => {
                    let changed = !matches!(last, LastDelivery::Absent);
                    *last = LastDelivery::Absent;
                    changed
                }
                SourceDelivery::Reading(Ok(value)) => {
                    let changed = !matches!(last, LastDelivery::Value(known) if known == value);
                    *last = LastDelivery::Value(value.clone());
                    changed
                }
                SourceDelivery::Reading(Err(_)) => {
                    *last = LastDelivery::Nothing;
                    true
                }
            };
            ready(Some(emit.then_some(delivery)))
        })
        .filter_map(ready)
}

enum LastDelivery<S> {
    Nothing,
    Absent,
    Value(S),
}
