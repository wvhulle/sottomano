//! Wired ethernet via `NetworkManager` (`nmcli`). Off the bar when no ethernet
//! device exists; dimmed when present but unplugged; otherwise a plain icon
//! with an interface/connection/IP/gateway tooltip.

use std::time::Duration;

use crate::{
    data_sources::{
        BarWidget, Source, TooltipBody,
        command::output,
        net_interfaces::{dash, terse_split},
        row,
    },
    widgets::icon::Icon,
};

#[derive(Debug, Clone)]
struct Device {
    name: String,
    connected: bool,
    connection: String,
    ip: String,
    gateway: String,
}

#[derive(Debug, Clone)]
pub struct Ethernet {
    devices: Vec<Device>,
}

impl Ethernet {
    fn connected_count(&self) -> usize {
        self.devices.iter().filter(|d| d.connected).count()
    }
}

impl BarWidget for Ethernet {
    fn icon(&self) -> Icon {
        Icon::Ethernet
    }

    fn disabled(&self) -> bool {
        self.connected_count() == 0
    }

    fn tooltip(&self) -> TooltipBody {
        let connected = self.connected_count();
        if connected == 0 {
            return TooltipBody::Table(
                self.devices
                    .iter()
                    .map(|d| row(&d.name, "disconnected"))
                    .collect(),
            );
        }
        // Prefix keys with the interface only when more than one link is up, so
        // a single connection reads with bare keys.
        let key = |device: &Device, label: &str| {
            if connected > 1 {
                format!("{} {label}", device.name)
            } else {
                label.to_string()
            }
        };
        let mut rows = Vec::new();
        for device in self.devices.iter().filter(|d| d.connected) {
            rows.push(row(&key(device, "Interface"), device.name.clone()));
            rows.push(row(&key(device, "Connection"), device.connection.clone()));
            rows.push(row(&key(device, "IP"), device.ip.clone()));
            rows.push(row(&key(device, "Gateway"), device.gateway.clone()));
        }
        TooltipBody::Table(rows)
    }
}

impl Source for Ethernet {
    const NAME: &'static str = "ethernet";
    const INTERVAL: Duration = Duration::from_secs(10);

    fn poll() -> Option<Self> {
        let status = output(
            "nmcli",
            &["-t", "-f", "TYPE,DEVICE,STATE,CONNECTION", "device", "status"],
        )?;
        let mut devices = Vec::new();
        for line in status.lines() {
            let fields = terse_split(line);
            if fields.first().map(String::as_str) != Some("ethernet") {
                continue;
            }
            let name = fields.get(1).cloned().unwrap_or_default();
            let connected = fields.get(2).map(String::as_str) == Some("connected");
            let mut connection = fields.get(3).cloned().unwrap_or_default();
            let (mut ip, mut gateway) = (String::from("—"), String::from("—"));
            if connected
                && let Some(detail) = output(
                    "nmcli",
                    &[
                        "-t",
                        "-f",
                        "IP4.ADDRESS,IP4.GATEWAY,GENERAL.CONNECTION",
                        "device",
                        "show",
                        &name,
                    ],
                )
            {
                for line in detail.lines() {
                    let Some((k, v)) = line.split_once(':') else {
                        continue;
                    };
                    match k {
                        "IP4.ADDRESS[1]" => ip = dash(v),
                        "IP4.GATEWAY" => gateway = dash(v),
                        "GENERAL.CONNECTION" => connection = dash(v),
                        _ => {}
                    }
                }
            }
            devices.push(Device {
                name,
                connected,
                connection: dash(&connection),
                ip,
                gateway,
            });
        }
        (!devices.is_empty()).then_some(Ethernet { devices })
    }
}
