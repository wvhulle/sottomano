//! Per-display hardware backlight over clightd's v1
//! `org.clightd.clightd.Backlight` interface (system bus), which addresses each
//! display individually by serial: internal panels appear under their
//! `/sys/class/backlight/*` name, external DDC/CI monitors under their EDID
//! serial. One backend covers both, and keeps clight's webcam auto-calibration
//! in the loop.
//!
//! A manual set pauses clight auto-calibration first (see [`clight`]) so the
//! webcam doesn't re-adjust the level. The list is dynamic (monitors come and
//! go), so this polls a `Vec` on a long interval — DDC/i2c reads are ~1 s and
//! flaky, so the UI leans on optimistic slider state between polls. Software
//! gamma dimming lives in [`gamma`](crate::data::source::gamma), a different
//! mechanism.
//!
//! [`clight`]: crate::data::source::clight

use std::{collections::HashSet, fs, time::Duration};

use iced::Subscription;
use serde::{Deserialize, Serialize};
use zbus::{blocking::Connection, proxy, zvariant::Type};

use crate::{
    data::source::{
        BarWidget, Glyph, TooltipBody, Unit, Value, clight,
        font_awesome::{FontAwesome, Solid},
        row,
    },
    message_subscriptions::poll_every,
};

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

/// How a display's backlight is driven, for the icon and tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A built-in panel (a `/sys/class/backlight` device).
    Internal,
    /// An external monitor driven over DDC/CI.
    External,
}

#[derive(Debug, Clone)]
pub struct Backlight {
    /// clightd's identifier: sysfs name (internal) or EDID serial (external).
    pub serial: String,
    pub kind: Kind,
    pub percent: i64,
}

impl Backlight {
    pub fn face(&self) -> FontAwesome {
        match self.kind {
            Kind::Internal => Solid::Laptop.into(),
            Kind::External => Solid::Desktop.into(),
        }
    }
}

impl BarWidget for Backlight {
    fn icon(&self) -> Glyph {
        self.face().into()
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let kind = match self.kind {
            Kind::Internal => "Internal",
            Kind::External => "External (DDC)",
        };
        TooltipBody::Table(vec![
            row("Display", self.serial.clone()),
            row("Type", kind),
            row("Brightness", format!("{}%", self.percent)),
        ])
    }
}

/// Re-poll cadence. Long, because DDC/i2c reads are slow and flaky; the
/// slider's optimistic [`Tracked`](crate::widgets::tracked::Tracked) state
/// covers the gap.
const INTERVAL: Duration = Duration::from_secs(10);

pub fn subscription() -> Subscription<Vec<Backlight>> {
    poll_every("backlight", INTERVAL, poll_all)
}

/// Every backlight clightd manages, classified and sorted by serial for a
/// stable slider order. Empty when clightd is unavailable.
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

/// `/sys/class/backlight` device names — clightd reports internal panels under
/// these, so anything else it lists is an external DDC monitor.
fn internal_names() -> HashSet<String> {
    fs::read_dir("/sys/class/backlight")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// Set one display (by serial) to `percent`, pausing clight auto-calibration
/// first so the webcam doesn't fight the change. Returns clightd's success
/// flag.
pub fn set(serial: &str, percent: i64) -> bool {
    let frac = f64::from(i32::try_from(percent.clamp(0, 100)).unwrap_or(0)) / 100.0;
    clight::pause_auto_calib();
    clightd_set(serial, frac)
}

/// Freeze every display at its current level (re-set through clightd), aborting
/// an in-flight clight ramp when switching to manual.
pub fn halt_ramp() {
    for Backlight {
        serial, percent, ..
    } in poll_all()
    {
        let frac = f64::from(i32::try_from(percent).unwrap_or(0)) / 100.0;
        let _ = clightd_set(&serial, frac);
    }
}

/// clightd v1 `Set(target, (smooth, step, timeout), serial) -> ok`; instant
/// (no smoothing) so the slider lands immediately.
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
