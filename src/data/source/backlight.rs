use std::{collections::HashSet, convert::Infallible, fs, future::ready, time::Duration};

use iced::futures::{Stream, StreamExt, stream::select};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;
use zbus::{blocking::Connection, proxy, zvariant::Type};

use crate::{
    data::source::{
        Delivery, PolledSource, Source,
        bus::{Bus, name_owned},
        clight, delivered, emit_delivery_changes, present_or_absent,
    },
    message_subscriptions::{restarting, ticked},
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

impl Source for Vec<Backlight> {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "backlight";
}

impl PolledSource for Vec<Backlight> {
    const INTERVAL: Duration = INTERVAL;

    async fn poll(_key: ()) -> Result<Self, Infallible> {
        Ok(spawn_blocking(poll_all).await.unwrap_or_default())
    }

    fn polls(key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        present_or_absent(name_owned(Bus::System, CLIGHTD_SERVICE), move || {
            emit_delivery_changes(delivered(
                restarting("backlight", backlight_changes).then(move |()| Self::poll(key)),
            ))
            .boxed()
        })
    }
}

async fn backlight_changes() -> zbus::Result<impl Stream<Item = ()>> {
    let connection = zbus::Connection::system().await?;
    let proxy = ClightdBacklightProxy::new(&connection).await?;
    let changes = proxy.receive_changed().await?.map(drop);
    Ok(select(ticked(INTERVAL, || ready(())), changes))
}

pub fn poll_all() -> Vec<Backlight> {
    let Ok(connection) = Connection::system() else {
        return Vec::new();
    };
    let Ok(proxy) = ClightdBacklightProxyBlocking::new(&connection) else {
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
