use iced::futures::{Stream, try_join};
use zbus::Connection;

use crate::data::source::net_interfaces::{nm, nm_readings};

pub const NAME: &str = "wifi";

#[derive(Debug, Clone)]
pub struct Link {
    pub ssid: Option<String>,
    pub signal: i64,
    pub frequency: Option<String>,
    pub security: Option<String>,
    pub mac: Option<String>,
    pub ipv4: Option<nm::AddressBlock>,
    pub ipv6: Option<nm::AddressBlock>,
    pub dns: Vec<String>,
    pub bitrate: u32,
    pub mtu: u32,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    pub device: String,
    pub link: Option<Link>,
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
