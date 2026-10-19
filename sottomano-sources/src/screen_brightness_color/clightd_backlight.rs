use std::{collections::HashSet, fs, future::ready, path::PathBuf, time::Duration};

use futures::{Stream, StreamExt, stream::select};
use serde::{Deserialize, Serialize};
use zbus::{blocking::Connection, proxy, proxy::CacheProperties, zvariant::Type};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    screen_brightness_color::clight_auto_brightness,
    stream_operators::{
        StreamOperators, connected_until_failure, read_once_then_per_burst, ticked,
    },
    system_protocol_clients::{
        dbus_desktop_buses::{DesktopBuses, name_owned},
        sysfs_attribute_reads::{list_dir, read_i64},
    },
};

const CLIGHTD_SERVICE: &str = "org.clightd.clightd";

#[proxy(
    interface = "org.clightd.clightd.Backlight",
    default_service = "org.clightd.clightd",
    default_path = "/org/clightd/clightd/Backlight"
)]
trait ClightdBacklight {
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

#[derive(Debug, Serialize, Type)]
struct Smooth {
    enabled: bool,
    step: f64,
    timeout: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacklightKind {
    Internal,
    External,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Backlight {
    pub serial: String,
    pub kind: BacklightKind,
    pub percent: i64,
}

const INTERVAL: Duration = Duration::from_secs(10);

impl ReadingSource for Vec<Backlight> {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "backlight";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(INTERVAL);

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let system = buses.system;
        name_owned(system.clone(), CLIGHTD_SERVICE).while_held(
            move || {
                connected_until_failure(backlight_readings(system.clone()))
                    .map(SourceDelivery::Reading)
                    .distinct_until_changed()
            },
            SourceDelivery::Absent,
        )
    }
}

async fn backlight_readings(
    system: zbus::Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Vec<Backlight>>>> {
    let proxy: ClightdBacklightProxy<'static> = proxy::Builder::new(&system)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let changes = proxy.receive_changed().await?.map(drop);
    Ok(read_once_then_per_burst(
        select(ticked(INTERVAL, || ready(())), changes),
        move || {
            let proxy = proxy.clone();
            async move { proxy.get_all("").await.map(backlights) }
        },
    ))
}

fn blocking_backlight_proxy() -> Option<ClightdBacklightProxyBlocking<'static>> {
    let connection = Connection::system().ok()?;
    zbus::blocking::proxy::Builder::new(&connection)
        .cache_properties(CacheProperties::No)
        .build()
        .ok()
}

pub fn poll_all() -> Vec<Backlight> {
    let Some(proxy) = blocking_backlight_proxy() else {
        return Vec::new();
    };
    proxy.get_all("").map(backlights).unwrap_or_default()
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
pub fn set(serial: &str, percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    clight_auto_brightness::pause_auto_calib();
    clightd_set(serial, frac)
}

pub fn halt_ramp() {
    for Backlight {
        serial, percent, ..
    } in poll_all()
    {
        let frac = f64::from(i32::try_from(percent).unwrap_or(0)) / 100.0;
        let _ = clightd_set(&serial, frac);
    }
}

fn clightd_set(serial: &str, frac: f64) -> bool {
    let Some(proxy) = blocking_backlight_proxy() else {
        return false;
    };
    proxy
        .set(
            frac,
            Smooth {
                enabled: false,
                step: 0.0,
                timeout: 0,
            },
            serial,
        )
        .unwrap_or(false)
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
