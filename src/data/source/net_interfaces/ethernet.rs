use iced::futures::{SinkExt, StreamExt, channel::mpsc::Sender};
use zbus::Connection;

use crate::data::source::{
    BarWidget, Glyph, TooltipBody,
    font_awesome::Solid,
    net_interfaces::{address_rows, dns_rows, group, link_stats_rows, nm},
    row,
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
            return TooltipBody::Table(
                self.links
                    .iter()
                    .map(|link| row(&link.name, "disconnected"))
                    .collect(),
            );
        }
        let mut rows = Vec::new();
        for link in self.links.iter().filter(|link| link.connected) {
            let key = |label: &str| {
                if connected > 1 {
                    format!("{} {label}", link.name)
                } else {
                    label.to_string()
                }
            };
            let mut identity = vec![row(&key("Interface"), link.name.clone())];
            if let Some(connection) = &link.connection {
                identity.push(row(&key("Connection"), connection.clone()));
            }
            if let Some(mac) = &link.mac {
                identity.push(row(&key("MAC"), mac.clone()));
            }
            group(&mut rows, identity);
            group(&mut rows, address_rows(&key, "IPv4", link.ipv4.as_ref()));
            group(&mut rows, address_rows(&key, "IPv6", link.ipv6.as_ref()));
            group(&mut rows, dns_rows(&key, &link.dns));
            group(
                &mut rows,
                link_stats_rows(&key, "Speed", link.speed, link.mtu),
            );
        }
        TooltipBody::Table(rows)
    }
}

async fn read(connection: &Connection) -> Option<Ethernet> {
    let mut links = Vec::new();
    for device in nm::devices(connection)
        .await
        .into_iter()
        .filter(|device| device.kind == nm::TYPE_ETHERNET)
    {
        if device.connected {
            let ip = nm::ip(connection, &device.path).await;
            links.push(Link {
                name: device.interface,
                connected: true,
                connection: nm::connection_id(connection, &device.path).await,
                mac: nm::hw_address(connection, &device.path).await,
                ipv4: ip.v4,
                ipv6: ip.v6,
                dns: ip.dns,
                speed: nm::wired_speed(connection, &device.path).await,
                mtu: nm::mtu(connection, &device.path).await,
            });
        } else {
            links.push(Link {
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
    }
    (!links.is_empty()).then_some(Ethernet { links })
}

pub async fn run(mut sink: Sender<Option<Ethernet>>) {
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
