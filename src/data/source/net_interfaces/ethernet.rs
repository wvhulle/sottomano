use iced::futures::{SinkExt, StreamExt, channel::mpsc::Sender};
use zbus::Connection;

use crate::data::source::{
    BarWidget, Glyph, TooltipBody, font_awesome::Solid, net_interfaces::nm, row,
};

pub const NAME: &str = "ethernet";

#[derive(Debug, Clone)]
struct Link {
    name: String,
    connected: bool,
    connection: String,
    ip: String,
    gateway: String,
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
        let key = |link: &Link, label: &str| {
            if connected > 1 {
                format!("{} {label}", link.name)
            } else {
                label.to_string()
            }
        };
        let mut rows = Vec::new();
        for link in self.links.iter().filter(|link| link.connected) {
            rows.push(row(&key(link, "Interface"), link.name.clone()));
            rows.push(row(&key(link, "Connection"), link.connection.clone()));
            rows.push(row(&key(link, "IP"), link.ip.clone()));
            rows.push(row(&key(link, "Gateway"), link.gateway.clone()));
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
        let (connection_id, ip, gateway) = if device.connected {
            let ip = nm::ip(connection, &device.path).await;
            (
                nm::connection_id(connection, &device.path).await,
                ip.address,
                ip.gateway,
            )
        } else {
            (
                nm::DASH.to_owned(),
                nm::DASH.to_owned(),
                nm::DASH.to_owned(),
            )
        };
        links.push(Link {
            name: device.interface,
            connected: device.connected,
            connection: connection_id,
            ip,
            gateway,
        });
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
