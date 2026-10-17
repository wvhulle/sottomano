use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    future::{Future, ready},
    hash::Hash,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use iced::futures::{
    Stream, StreamExt,
    stream::{self, BoxStream},
};
use tokio::task::spawn_blocking;

use crate::message_subscriptions::ticked;

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

pub trait Source: Clone + Debug + Sized + Send + 'static {
    type Error: Clone + Debug + Display + Send + 'static;
    type Key: Clone + Hash + Send + Sync + 'static;

    const NAME: &'static str;
}

#[derive(Debug, Clone)]
pub enum Delivery<S: Source> {
    Absent,
    Reading(Result<S, S::Error>),
}

pub trait PolledSource: Source + PartialEq {
    const INTERVAL: Duration;

    fn poll(key: Self::Key) -> impl Future<Output = Result<Self, Self::Error>> + Send;

    fn polls(key: Self::Key) -> impl Stream<Item = Delivery<Self>> + Send {
        ticked_polls::<Self>(key)
    }
}

pub trait LiveSource: Source {
    fn events(key: Self::Key) -> impl Stream<Item = Delivery<Self>> + Send;
}

pub fn ticked_polls<S: PolledSource>(key: S::Key) -> impl Stream<Item = Delivery<S>> + Send {
    emit_delivery_changes(ticked(S::INTERVAL, move || S::poll(key.clone())).map(Delivery::Reading))
}

pub fn delivered<S: Source>(
    readings: impl Stream<Item = Result<S, S::Error>> + Send,
) -> impl Stream<Item = Delivery<S>> + Send {
    readings.map(Delivery::Reading)
}

pub fn present_or_absent<S: Source>(
    presence: impl Stream<Item = bool> + Send + 'static,
    readings: impl FnMut() -> BoxStream<'static, Delivery<S>> + Send + 'static,
) -> impl Stream<Item = Delivery<S>> + Send {
    while_present(presence, readings).map(|delivery| delivery.unwrap_or(Delivery::Absent))
}

pub fn while_present<T: Send + 'static>(
    presence: impl Stream<Item = bool> + Send + 'static,
    readings: impl FnMut() -> BoxStream<'static, T> + Send + 'static,
) -> impl Stream<Item = Option<T>> + Send {
    WhilePresent {
        presence: presence.boxed(),
        presence_ended: false,
        start: Box::new(readings),
        inner: None,
    }
}

struct WhilePresent<T> {
    presence: BoxStream<'static, bool>,
    presence_ended: bool,
    start: Box<dyn FnMut() -> BoxStream<'static, T> + Send>,
    inner: Option<BoxStream<'static, T>>,
}

impl<T> Stream for WhilePresent<T> {
    type Item = Option<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        while !this.presence_ended {
            match this.presence.poll_next_unpin(cx) {
                Poll::Ready(Some(true)) => {
                    if this.inner.is_none() {
                        this.inner = Some((this.start)());
                    }
                }
                Poll::Ready(Some(false)) => {
                    if this.inner.take().is_some() {
                        return Poll::Ready(Some(None));
                    }
                }
                Poll::Ready(None) => this.presence_ended = true,
                Poll::Pending => break,
            }
        }
        let Some(inner) = this.inner.as_mut() else {
            return if this.presence_ended {
                Poll::Ready(None)
            } else {
                Poll::Pending
            };
        };
        match inner.poll_next_unpin(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(Some(item))),
            Poll::Ready(None) => {
                this.inner = None;
                if this.presence_ended {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub fn all_of(
    conditions: impl IntoIterator<Item = BoxStream<'static, bool>>,
) -> impl Stream<Item = bool> + Send {
    let tagged: Vec<BoxStream<'static, (usize, bool)>> = conditions
        .into_iter()
        .enumerate()
        .map(|(index, condition)| condition.map(move |held| (index, held)).boxed())
        .collect();
    let latest: Vec<Option<bool>> = vec![None; tagged.len()];
    stream::select_all(tagged)
        .scan(latest, |latest, (index, held)| {
            latest[index] = Some(held);
            ready(Some(
                latest
                    .iter()
                    .map(|known| known.map(|held| held.then_some(())))
                    .collect::<Option<Option<()>>>()
                    .map(|all| all.is_some()),
            ))
        })
        .filter_map(ready)
}

pub fn settled(present: bool) -> impl Stream<Item = bool> + Send {
    stream::iter([present])
}

pub fn emit_delivery_changes<S: PolledSource>(
    deliveries: impl Stream<Item = Delivery<S>> + Send,
) -> impl Stream<Item = Delivery<S>> + Send {
    deliveries
        .scan(LastDelivery::Nothing, |last, delivery| {
            let emit = match &delivery {
                Delivery::Absent => {
                    let changed = !matches!(last, LastDelivery::Absent);
                    *last = LastDelivery::Absent;
                    changed
                }
                Delivery::Reading(Ok(value)) => {
                    let changed = !matches!(last, LastDelivery::Value(known) if known == value);
                    *last = LastDelivery::Value(value.clone());
                    changed
                }
                Delivery::Reading(Err(_)) => {
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
