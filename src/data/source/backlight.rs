use std::{collections::HashSet, fs, time::Duration};

use iced::Subscription;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;
use zbus::{blocking::Connection, proxy, zvariant::Type};

use crate::{data::source::clight, message_subscriptions::poll_every};

#[proxy(
    interface = "org.clightd.clightd.Backlight",
    default_service = "org.clightd.clightd",
    default_path = "/org/clightd/clightd/Backlight"
)]
trait ClightdBacklight {
    fn get_all(&self, serial: &str) -> zbus::Result<Vec<BacklightReading>>;
    fn set(&self, target: f64, smooth: Smooth, serial: &str) -> zbus::Result<bool>;
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
pub enum Kind {
    Internal,
    External,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Backlight {
    pub serial: String,
    pub kind: Kind,
    pub percent: i64,
}

const INTERVAL: Duration = Duration::from_secs(10);

pub fn subscription() -> Subscription<Vec<Backlight>> {
    poll_every("backlight", INTERVAL, polled_backlights)
}

async fn polled_backlights() -> Vec<Backlight> {
    spawn_blocking(poll_all).await.unwrap_or_default()
}

pub fn poll_all() -> Vec<Backlight> {
    let Ok(connection) = Connection::system() else {
        return Vec::new();
    };
    let Ok(proxy) = ClightdBacklightProxyBlocking::new(&connection) else {
        return Vec::new();
    };
    let Ok(readings) = proxy.get_all("") else {
        return Vec::new();
    };
    let internal = internal_names();
    let mut list: Vec<Backlight> = readings
        .into_iter()
        .map(|reading| {
            let kind = if internal.contains(&reading.serial) {
                Kind::Internal
            } else {
                Kind::External
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

pub fn set(serial: &str, percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    clight::pause_auto_calib();
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
    let Ok(connection) = Connection::system() else {
        return false;
    };
    let Ok(proxy) = ClightdBacklightProxyBlocking::new(&connection) else {
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
