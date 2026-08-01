use iced::futures::{SinkExt, StreamExt, channel::mpsc::Sender};
use zbus::Connection;

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value,
    font_awesome::Solid,
    net_interfaces::{address_rows, dns_rows, group, link_stats_rows, nm, signal_level},
    row, row_pair,
};

pub const NAME: &str = "wifi";

#[derive(Debug, Clone)]
struct Link {
    ssid: Option<String>,
    signal: i64,
    frequency: Option<String>,
    security: Option<String>,
    mac: Option<String>,
    ipv4: Option<nm::AddressBlock>,
    ipv6: Option<nm::AddressBlock>,
    dns: Vec<String>,
    bitrate: u32,
    mtu: u32,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    device: String,
    link: Option<Link>,
}

impl BarWidget for Wifi {
    fn icon(&self) -> Glyph {
        Solid::Wifi.into()
    }

    fn disabled(&self) -> bool {
        self.link.is_none()
    }

    fn level(&self) -> Level {
        self.link
            .as_ref()
            .map_or(Level::Ok, |link| signal_level(link.signal))
    }

    fn value(&self) -> Option<Value> {
        self.link.as_ref().map(|link| Value::Gauge {
            percent: link.signal,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let Some(link) = &self.link else {
            return TooltipBody::Table(vec![row("WiFi", "disconnected")]);
        };
        let key = |label: &str| label.to_string();
        let mut identity = Vec::new();
        if let Some(ssid) = &link.ssid {
            identity.push(row("SSID", ssid.clone()));
        }
        identity.push(match &link.security {
            Some(security) => row_pair(
                "Signal",
                format!("{}%", link.signal),
                "Security",
                security.clone(),
            ),
            None => row("Signal", format!("{}%", link.signal)),
        });
        if let Some(frequency) = &link.frequency {
            identity.push(row("Frequency", frequency.clone()));
        }
        identity.push(row("Interface", self.device.clone()));
        if let Some(mac) = &link.mac {
            identity.push(row("MAC", mac.clone()));
        }
        let mut rows = Vec::new();
        group(&mut rows, identity);
        group(&mut rows, address_rows(&key, "IPv4", link.ipv4.as_ref()));
        group(&mut rows, address_rows(&key, "IPv6", link.ipv6.as_ref()));
        group(&mut rows, dns_rows(&key, &link.dns));
        group(
            &mut rows,
            link_stats_rows(&key, "Bitrate", link.bitrate / 1000, link.mtu),
        );
        TooltipBody::Table(rows)
    }
}

async fn read(connection: &Connection) -> Option<Wifi> {
    let device = nm::devices(connection)
        .await
        .into_iter()
        .find(|device| device.kind == nm::TYPE_WIFI)?;
    if !device.connected {
        return Some(Wifi {
            device: device.interface,
            link: None,
        });
    }
    let link = match nm::access_point(connection, &device.path).await {
        Some(ap) => {
            let ip = nm::ip(connection, &device.path).await;
            Some(Link {
                ssid: ap.ssid,
                signal: ap.signal,
                frequency: ap.frequency,
                security: ap.security,
                mac: nm::hw_address(connection, &device.path).await,
                ipv4: ip.v4,
                ipv6: ip.v6,
                dns: ip.dns,
                bitrate: nm::wireless_rate(connection, &device.path).await,
                mtu: nm::mtu(connection, &device.path).await,
            })
        }
        None => None,
    };
    Some(Wifi {
        device: device.interface,
        link,
    })
}

pub async fn run(mut sink: Sender<Option<Wifi>>) {
    let Ok(connection) = Connection::system().await else {
        return;
    };
    if sink.send(read(&connection).await).await.is_err() {
        return;
    }
    let Ok(mut signals) = nm::changes(&connection).await else {
        return;
    };
    while signals.next().await.is_some() {
        if sink.send(read(&connection).await).await.is_err() {
            return;
        }
    }
}
