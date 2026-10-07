use iced::futures::{Stream, try_join};
use uom::si::f64::{Frequency, InformationRate};
use zbus::Connection;

use crate::data::source::{
    hardware::Adapter,
    net_interfaces::{nm, nm_readings},
};

pub const NAME: &str = "wifi";

#[derive(Debug, Clone)]
pub struct Link {
    pub ssid: Option<String>,
    pub signal: i64,
    pub frequency: Frequency,
    pub security: Option<String>,
    pub mac: Option<String>,
    pub ipv4: Option<nm::AddressBlock>,
    pub ipv6: Option<nm::AddressBlock>,
    pub dns: Vec<String>,
    pub bitrate: InformationRate,
    pub mtu: u32,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    pub device: String,
    pub adapter: Option<Adapter>,
    pub link: Option<Link>,
}

impl Wifi {
    fn unlinked(device: nm::DeviceInfo) -> Self {
        Wifi {
            adapter: Adapter::of_net_interface(&device.interface),
            device: device.interface,
            link: None,
        }
    }
}

async fn read(connection: Connection) -> zbus::Result<Option<Wifi>> {
    let Some(device) = nm::devices(&connection)
        .await?
        .into_iter()
        .find(|device| device.kind == nm::DeviceKind::Wifi)
    else {
        return Ok(None);
    };
    if !device.connected {
        return Ok(Some(Wifi::unlinked(device)));
    }
    let Some(ap) = nm::access_point(&connection, &device).await? else {
        return Ok(Some(Wifi::unlinked(device)));
    };
    let (ip, mac, bitrate, mtu) = try_join!(
        nm::ip(&connection, &device),
        nm::hw_address(&device),
        nm::wireless_rate(&connection, &device),
        nm::mtu(&device)
    )?;
    Ok(Some(Wifi {
        adapter: Adapter::of_net_interface(&device.interface),
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
