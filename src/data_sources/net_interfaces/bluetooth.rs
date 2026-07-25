//! Bluetooth adapter state via `bluetoothctl`. Absent adapter → off the bar;
//! powered off → a dimmed icon; powered on → a connected-count badge and, when
//! any connected device reports RSSI, a link-quality gauge.

use std::time::Duration;

use crate::{
    data_sources::{
        BarWidget, Level, Source, TooltipBody, Unit, Value,
        command::output,
        net_interfaces::signal_level,
        row,
    },
    widgets::icon::Icon,
};

#[derive(Debug, Clone)]
pub struct Bluetooth {
    powered: bool,
    connected: Vec<String>,
    /// Best link quality (0..100) across connected devices that report RSSI.
    signal: Option<i64>,
}

/// RSSI in dBm to a 0..100 quality, odin's `rssi-to-quality`: clamp to
/// [-90, -40] and map linearly.
fn rssi_quality(dbm: i64) -> i64 {
    (dbm.clamp(-90, -40) + 90) * 2
}

/// The dBm value on a `bluetoothctl info` RSSI line, which prints either
/// `RSSI: -72` or `RSSI: 0xffffffb8 (-72)`. Prefer the parenthesised decimal;
/// fall back to the first signed integer token.
fn parse_rssi(info: &str) -> Option<i64> {
    let line = info.lines().find(|l| l.trim_start().starts_with("RSSI:"))?;
    if let Some(start) = line.find('(') {
        let inner = &line[start + 1..];
        if let Some(end) = inner.find(')') {
            return inner[..end].trim().parse().ok();
        }
    }
    line.split_whitespace()
        .find_map(|token| token.parse::<i64>().ok())
}

impl BarWidget for Bluetooth {
    fn icon(&self) -> Icon {
        Icon::BluetoothB
    }

    fn disabled(&self) -> bool {
        !self.powered
    }

    fn badge(&self) -> Option<u32> {
        if !self.powered {
            return None;
        }
        u32::try_from(self.connected.len())
            .ok()
            .filter(|count| *count > 0)
    }

    fn level(&self) -> Level {
        self.signal.map_or(Level::Ok, signal_level)
    }

    fn value(&self) -> Option<Value> {
        self.signal.map(|percent| Value::Gauge {
            percent,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        if !self.powered {
            return TooltipBody::Table(vec![row("Bluetooth", "off")]);
        }
        let summary = if self.connected.is_empty() {
            "on".to_string()
        } else {
            format!("{} connected", self.connected.len())
        };
        let mut rows = vec![row("Bluetooth", summary)];
        for (index, name) in self.connected.iter().enumerate() {
            rows.push(row(&format!("Device {}", index + 1), name.clone()));
        }
        TooltipBody::Table(rows)
    }
}

impl Source for Bluetooth {
    const NAME: &'static str = "bluetooth";
    const INTERVAL: Duration = Duration::from_secs(5);

    fn poll() -> Option<Self> {
        let show = output("bluetoothctl", &["show"])?;
        // A `show` with no adapter block prints nothing usable; treat an empty
        // response as no adapter.
        if show.trim().is_empty() {
            return None;
        }
        let powered = show.lines().any(|l| l.trim() == "Powered: yes");
        if !powered {
            return Some(Bluetooth {
                powered: false,
                connected: Vec::new(),
                signal: None,
            });
        }
        let devices = output("bluetoothctl", &["devices", "Connected"]).unwrap_or_default();
        let mut connected = Vec::new();
        let mut best: Option<i64> = None;
        for line in devices.lines() {
            // `Device <MAC> <name>` — name may contain spaces.
            let Some(rest) = line.trim().strip_prefix("Device ") else {
                continue;
            };
            let Some((mac, name)) = rest.split_once(' ') else {
                continue;
            };
            connected.push(name.to_string());
            if let Some(info) = output("bluetoothctl", &["info", mac])
                && let Some(dbm) = parse_rssi(&info)
            {
                let quality = rssi_quality(dbm);
                best = Some(best.map_or(quality, |b| b.max(quality)));
            }
        }
        Some(Bluetooth {
            powered: true,
            connected,
            signal: best,
        })
    }
}
