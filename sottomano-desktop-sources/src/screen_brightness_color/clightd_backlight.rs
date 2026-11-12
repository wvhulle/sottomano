use std::{collections::HashSet, fs, future::ready, path::PathBuf, time::Duration};

use futures::{Stream, StreamExt, stream};
use futures_rx::RxExt;
use serde::{Deserialize, Serialize};
use zbus::{proxy, proxy::CacheProperties, zvariant::Type};

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::PeriodicPolling,
    system_protocol_clients::{
        dbus_desktop_buses::{DesktopBuses, name_owned},
        sysfs_attribute_reads::{list_dir, read_i64},
    },
};

const CLIGHTD_SERVICE: &str = "org.clightd.clightd";

#[proxy(
    interface = "org.clightd.clightd.Backlight",
    default_service = "org.clightd.clightd",
    default_path = "/org/clightd/clightd/Backlight",
    gen_blocking = false
)]
pub trait ClightdBacklight {
    fn get_all(&self, serial: &str) -> zbus::Result<Vec<BacklightReading>>;
    fn set(&self, target: f64, smooth: Smooth, serial: &str) -> zbus::Result<bool>;
    #[zbus(signal)]
    fn changed(&self, serial: &str, fraction: f64) -> zbus::Result<()>;
}

#[derive(Debug, Deserialize, Type)]
struct BacklightReading {
    serial: String,
    fraction: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacklightKind {
    Internal,
    External,
}

#[derive(Debug, Serialize, Type)]
pub struct Smooth {
    pub enabled: bool,
    pub step: f64,
    pub timeout: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Backlight {
    pub serial: String,
    pub kind: BacklightKind,
    pub percent: i64,
}

const BACKLIGHT_RECHECK: PeriodicPolling = PeriodicPolling::every(Duration::from_secs(10));

impl SnapshotSource for Vec<Backlight> {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Backlight";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        name_owned(system.clone(), CLIGHTD_SERVICE).while_available(
            move || backlight_deliveries(system.clone()).distinct_until_changed(),
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

struct BacklightWatch<Changes> {
    proxy: ClightdBacklightProxy<'static>,
    changes: Changes,
}

fn backlight_deliveries(
    system: zbus::Connection,
) -> impl Stream<Item = SnapshotDelivery<Vec<Backlight>>> + Send {
    stream::once(watched_backlights(system)).flat_map(|watched| match watched {
        Ok(BacklightWatch { proxy, changes }) => BACKLIGHT_RECHECK
            .triggered_deliveries(changes, move || {
                let proxy = proxy.clone();
                async move { proxy.get_all("").await.map(backlights) }
            })
            .left_stream(),
        Err(failure) => {
            stream::once(ready(SnapshotDelivery::live_push(Err(failure)))).right_stream()
        }
    })
}

async fn watched_backlights(
    system: zbus::Connection,
) -> zbus::Result<BacklightWatch<impl Stream<Item = ()> + Send + 'static>> {
    let proxy: ClightdBacklightProxy<'static> = proxy::Builder::new(&system)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let changes = proxy.receive_changed().await?.map(drop);
    Ok(BacklightWatch { proxy, changes })
}

pub async fn system_backlight_proxy() -> zbus::Result<ClightdBacklightProxy<'static>> {
    let system = zbus::Connection::system().await?;
    proxy::Builder::new(&system)
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

pub async fn poll_all() -> Vec<Backlight> {
    let Ok(proxy) = system_backlight_proxy().await else {
        return Vec::new();
    };
    proxy.get_all("").await.map(backlights).unwrap_or_default()
}

fn backlights(readings: Vec<BacklightReading>) -> Vec<Backlight> {
    let internal = internal_names();
    let mut list: Vec<Backlight> = readings
        .into_iter()
        .map(|reading| {
            let kind = if internal.contains(&reading.serial) {
                BacklightKind::Internal
            } else {
                BacklightKind::External
            };
            #[allow(clippy::cast_possible_truncation)]
            let percent = (reading.fraction * 100.0).round() as i64;
            Backlight {
                serial: reading.serial,
                kind,
                percent,
            }
        })
        .collect();
    list.sort_by(|a, b| a.serial.cmp(&b.serial));
    list
}

fn internal_names() -> HashSet<String> {
    fs::read_dir("/sys/class/backlight")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

#[must_use]
pub fn internal_brightness_frac() -> Option<f64> {
    let panel = first_backlight()?;
    let current = read_i64(panel.join("brightness"))?;
    let max = read_i64(panel.join("max_brightness")).filter(|m| *m > 0)?;
    Some((current as f64 / max as f64).clamp(0.0, 1.0))
}

fn first_backlight() -> Option<PathBuf> {
    let mut panels = list_dir("/sys/class/backlight");
    panels.sort();
    panels.into_iter().next()
}
