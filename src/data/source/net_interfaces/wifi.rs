use iced::futures::{Stream, try_join};
use zbus::Connection;

use crate::data::source::{
    BarWidget, Glyph, Level, TooltipBody, Unit, Value,
    font_awesome::Solid,
    net_interfaces::{
        address_section, dns_section, link_stats_section, nm, nm_readings, signal_level,
    },
    row, section,
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
            return TooltipBody::rows(vec![row("WiFi", "disconnected")]);
        };
        let key = |label: &str| label.to_string();
        let mut identity = Vec::new();
        if let Some(ssid) = &link.ssid {
            identity.push(row("Network Name (SSID)", ssid.clone()));
        }
        identity.push(row("Signal Strength", format!("{}%", link.signal)));
        if let Some(security) = &link.security {
            identity.push(row("Security Type", security.clone()));
        }
        if let Some(frequency) = &link.frequency {
            identity.push(row("Frequency Band", frequency.clone()));
        }
        identity.push(row("Interface Name", self.device.clone()));
        if let Some(mac) = &link.mac {
            identity.push(row("MAC Address", mac.clone()));
        }
        let sections = vec![
            section("Identity", identity),
            address_section(&key, "IPv4", link.ipv4.as_ref()),
            address_section(&key, "IPv6", link.ipv6.as_ref()),
            dns_section(&key, &link.dns),
            link_stats_section(&key, "Radio", "Radio Speed", link.bitrate / 1000, link.mtu),
        ];
        TooltipBody::Sections(sections)
    }
}

async fn read(connection: Connection) -> zbus::Result<Option<Wifi>> {
    let Some(device) = nm::devices(&connection)
        .await?
        .into_iter()
        .find(|device| device.kind == nm::TYPE_WIFI)
    else {
        return Ok(None);
    };
    if !device.connected {
        return Ok(Some(Wifi {
            device: device.interface,
            link: None,
        }));
    }
    let Some(ap) = nm::access_point(&connection, &device.path).await? else {
        return Ok(Some(Wifi {
            device: device.interface,
            link: None,
        }));
    };
    let (ip, mac, bitrate, mtu) = try_join!(
        nm::ip(&connection, &device.path),
        nm::hw_address(&connection, &device.path),
        nm::wireless_rate(&connection, &device.path),
        nm::mtu(&connection, &device.path)
    )?;
    Ok(Some(Wifi {
        device: device.interface,
        link: Some(Link {
            ssid: ap.ssid,
            signal: ap.signal,
            frequency: ap.frequency,
            security: ap.security,
            mac,
            ipv4: ip.v4,
            ipv6: ip.v6,
            dns: ip.dns,
            bitrate,
            mtu,
        }),
    }))
}

pub fn readings() -> impl Stream<Item = Option<Wifi>> {
    nm_readings(NAME, read)
}
