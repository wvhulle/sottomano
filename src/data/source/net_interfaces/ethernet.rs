use iced::futures::{Stream, future::try_join_all, try_join};
use uom::si::f64::InformationRate;
use zbus::Connection;

use crate::data::source::net_interfaces::{nm, nm_readings};

pub const NAME: &str = "ethernet";

#[derive(Debug, Clone)]
pub struct Link {
    pub name: String,
    pub connected: bool,
    pub connection: Option<String>,
    pub mac: Option<String>,
    pub ipv4: Option<nm::AddressBlock>,
    pub ipv6: Option<nm::AddressBlock>,
    pub dns: Vec<String>,
    pub speed: InformationRate,
    pub mtu: u32,
}

#[derive(Debug, Clone)]
pub struct Ethernet {
    pub links: Vec<Link>,
}

impl Ethernet {
    pub fn connected_count(&self) -> usize {
        self.links.iter().filter(|link| link.connected).count()
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
            speed: InformationRate::default(),
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
