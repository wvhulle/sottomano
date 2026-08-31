use iced::futures::{Stream, future::try_join_all, try_join};
use zbus::Connection;

use crate::data::source::{
    BarWidget, Glyph, TooltipBody,
    font_awesome::Solid,
    net_interfaces::{address_section, dns_section, link_stats_section, nm, nm_readings},
    row, section,
};

pub const NAME: &str = "ethernet";

#[derive(Debug, Clone)]
struct Link {
    name: String,
    connected: bool,
    connection: Option<String>,
    mac: Option<String>,
    ipv4: Option<nm::AddressBlock>,
    ipv6: Option<nm::AddressBlock>,
    dns: Vec<String>,
    speed: u32,
    mtu: u32,
}

#[derive(Debug, Clone)]
pub struct Ethernet {
    links: Vec<Link>,
}

impl Ethernet {
    fn connected_count(&self) -> usize {
        self.links.iter().filter(|link| link.connected).count()
    }
}

impl BarWidget for Ethernet {
    fn icon(&self) -> Glyph {
        Solid::Ethernet.into()
    }

    fn disabled(&self) -> bool {
        self.connected_count() == 0
    }

    fn tooltip(&self) -> TooltipBody {
        let connected = self.connected_count();
        if connected == 0 {
            return TooltipBody::rows(
                self.links
                    .iter()
                    .map(|link| row(&link.name, "disconnected"))
                    .collect(),
            );
        }
        let mut sections = Vec::new();
        for link in self.links.iter().filter(|link| link.connected) {
            let key = |label: &str| {
                if connected > 1 {
                    format!("{} {label}", link.name)
                } else {
                    label.to_string()
                }
            };
            let mut identity = vec![row("Interface Name", link.name.clone())];
            if let Some(connection) = &link.connection {
                identity.push(row("Network Profile", connection.clone()));
            }
            if let Some(mac) = &link.mac {
                identity.push(row("MAC Address", mac.clone()));
            }
            sections.push(section(key("Identity"), identity));
            sections.push(address_section(&key, "IPv4", link.ipv4.as_ref()));
            sections.push(address_section(&key, "IPv6", link.ipv6.as_ref()));
            sections.push(dns_section(&key, &link.dns));
            sections.push(link_stats_section(
                &key,
                "Link",
                "Link Speed",
                link.speed,
                link.mtu,
            ));
        }
        TooltipBody::Sections(sections)
    }
}

async fn link(connection: &Connection, device: nm::DeviceInfo) -> zbus::Result<Link> {
    if !device.connected {
        return Ok(Link {
            name: device.interface,
            connected: false,
            connection: None,
            mac: None,
            ipv4: None,
            ipv6: None,
            dns: Vec::new(),
            speed: 0,
            mtu: 0,
        });
    }
    let (ip, connection_id, mac, speed, mtu) = try_join!(
        nm::ip(connection, &device.path),
        nm::connection_id(connection, &device.path),
        nm::hw_address(connection, &device.path),
        nm::wired_speed(connection, &device.path),
        nm::mtu(connection, &device.path)
    )?;
    Ok(Link {
        name: device.interface,
        connected: true,
        connection: connection_id,
        mac,
        ipv4: ip.v4,
        ipv6: ip.v6,
        dns: ip.dns,
        speed,
        mtu,
    })
}

async fn read(connection: Connection) -> zbus::Result<Option<Ethernet>> {
    let devices = nm::devices(&connection)
        .await?
        .into_iter()
        .filter(|device| device.kind == nm::TYPE_ETHERNET);
    let links = try_join_all(devices.map(|device| link(&connection, device))).await?;
    Ok((!links.is_empty()).then_some(Ethernet { links }))
}

pub fn readings() -> impl Stream<Item = Option<Ethernet>> {
    nm_readings(NAME, read)
}
