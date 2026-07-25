//! Wi-Fi via `NetworkManager` (`nmcli`). Off the bar when no wireless device
//! exists; dimmed when the radio is up but not associated; otherwise an icon
//! with a signal-quality gauge and an SSID/signal/security tooltip.

use std::time::Duration;

use crate::{
    data_sources::{
        BarWidget, Level, Source, TooltipBody, Unit, Value,
        command::output,
        net_interfaces::{dash, signal_level, terse_split},
        row,
    },
    widgets::icon::Icon,
};

#[derive(Debug, Clone)]
struct Connection {
    ssid: String,
    signal: i64,
    frequency: String,
    security: String,
    ip: String,
    gateway: String,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    device: String,
    connection: Option<Connection>,
}

impl BarWidget for Wifi {
    fn icon(&self) -> Icon {
        Icon::Wifi
    }

    fn disabled(&self) -> bool {
        self.connection.is_none()
    }

    fn level(&self) -> Level {
        self.connection
            .as_ref()
            .map_or(Level::Ok, |c| signal_level(c.signal))
    }

    fn value(&self) -> Option<Value> {
        self.connection.as_ref().map(|c| Value::Gauge {
            percent: c.signal,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        match &self.connection {
            None => TooltipBody::Table(vec![row("WiFi", "disconnected")]),
            Some(c) => TooltipBody::Table(vec![
                row("SSID", c.ssid.clone()),
                row("Signal", format!("{}%", c.signal)),
                row("Frequency", c.frequency.clone()),
                row("Security", c.security.clone()),
                row("Interface", self.device.clone()),
                row("IP", c.ip.clone()),
                row("Gateway", c.gateway.clone()),
            ]),
        }
    }
}

impl Source for Wifi {
    const NAME: &'static str = "wifi";
    const INTERVAL: Duration = Duration::from_secs(10);

    fn poll() -> Option<Self> {
        let status = output(
            "nmcli",
            &["-t", "-f", "TYPE,DEVICE,STATE,CONNECTION", "device", "status"],
        )?;
        let device_line = status
            .lines()
            .map(terse_split)
            .find(|f| f.first().map(String::as_str) == Some("wifi"))?;
        let device = device_line.get(1).cloned().unwrap_or_default();
        let connected = device_line.get(2).map(String::as_str) == Some("connected");
        if !connected {
            return Some(Wifi {
                device,
                connection: None,
            });
        }
        Some(Wifi {
            connection: active_connection(&device),
            device,
        })
    }
}

/// The associated network's details on `device`, from `nmcli dev wifi list` (the
/// active row) plus the device's IP config.
fn active_connection(device: &str) -> Option<Connection> {
    let list = output(
        "nmcli",
        &[
            "-t",
            "-f",
            "active,ssid,signal,freq,security",
            "dev",
            "wifi",
            "list",
            "ifname",
            device,
        ],
    )?;
    let fields = list
        .lines()
        .map(terse_split)
        .find(|f| f.first().map(String::as_str) == Some("yes"))?;
    let ssid = dash(fields.get(1).map_or("", String::as_str));
    let signal = fields.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let frequency = dash(fields.get(3).map_or("", String::as_str));
    let security = dash(fields.get(4).map_or("", String::as_str));

    let (mut ip, mut gateway) = (String::from("—"), String::from("—"));
    if let Some(detail) = output(
        "nmcli",
        &[
            "-t",
            "-f",
            "IP4.ADDRESS,IP4.GATEWAY",
            "device",
            "show",
            device,
        ],
    ) {
        for line in detail.lines() {
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            match k {
                "IP4.ADDRESS[1]" => ip = dash(v),
                "IP4.GATEWAY" => gateway = dash(v),
                _ => {}
            }
        }
    }
    Some(Connection {
        ssid,
        signal,
        frequency,
        security,
        ip,
        gateway,
    })
}
