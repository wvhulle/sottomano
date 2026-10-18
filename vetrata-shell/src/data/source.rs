use std::{
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    future::Future,
    hash::Hash,
    time::Duration,
};

use iced::futures::{Stream, StreamExt, stream};
use tokio::task::spawn_blocking;

use crate::{
    stream_operators::{StreamOperators, ticked},
    widgets::tooltip_body::Section,
};

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
    ticked(every, move || poll(inputs.clone()))
        .map(SourceDelivery::Reading)
        .distinct_until_changed()
        .right_stream()
}

impl<S: ReadingSource + PartialEq> PartialEq for SourceDelivery<S> {
    fn eq(&self, other: &Self) -> bool {
        match self {
            SourceDelivery::Absent => matches!(other, SourceDelivery::Absent),
            SourceDelivery::Reading(Ok(value)) => {
                matches!(other, SourceDelivery::Reading(Ok(known)) if known == value)
            }
            SourceDelivery::Reading(Err(_)) => false,
        }
    }
}
