use std::convert::Infallible;

use iced::futures::{Stream, StreamExt, try_join};
use uom::si::f64::{Frequency, InformationRate};
use zbus::Connection;

use crate::data::source::{
    Delivery, LiveSource, Source,
    hardware::Adapter,
    net_interfaces::{ReadingError, device_or_absent, nm, nm_readings},
};

pub const NAME: &str = "wifi";

#[derive(Debug, Clone)]
pub struct Link {
    pub ssid: Option<String>,
    pub signal: i64,
    pub frequency: Frequency,
    pub security: String,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    pub device: String,
    pub link: Option<Link>,
}

#[derive(Debug, Clone)]
pub struct Addressing {
    pub mac: String,
    pub ipv4: Option<nm::AddressBlock>,
    pub ipv6: Option<nm::AddressBlock>,
    pub dns: Vec<String>,
    pub bitrate: InformationRate,
    pub mtu: u32,
}

#[derive(Debug, Clone)]
pub struct WifiDetails {
    pub adapter: Option<Adapter>,
    pub addressing: Option<Addressing>,
}

async fn wifi_device(connection: &Connection) -> Result<nm::DeviceInfo, ReadingError> {
    nm::devices(connection)
        .await?
        .into_iter()
        .find(|device| device.kind == nm::DeviceKind::Wifi)
        .ok_or(ReadingError::Absent)
}

async fn read(connection: Connection) -> Result<Wifi, ReadingError> {
    let device = wifi_device(&connection).await?;
    let ap_path = if device.connected {
        Some(nm::active_access_point(&connection, &device).await?)
    } else {
        None
    };
    let link = match ap_path.filter(|path| !nm::is_unset(path)) {
        Some(path) => {
            let ap = nm::access_point(&connection, path).await?;
            Some(Link {
                ssid: ap.ssid,
                signal: ap.signal,
                frequency: ap.frequency,
                security: ap.security,
            })
        }
        None => None,
    };
    Ok(Wifi {
        device: device.interface,
        link,
    })
}

async fn read_details(connection: Connection) -> Result<WifiDetails, ReadingError> {
    let device = wifi_device(&connection).await?;
    let addressing = if device.connected {
        let (ip, mac, bitrate, mtu) = try_join!(
            nm::ip(&connection, &device),
            nm::hw_address(&device),
            nm::wireless_rate(&connection, &device),
            nm::mtu(&device)
        )?;
        Some(Addressing {
            mac,
            ipv4: ip.v4,
            ipv6: ip.v6,
            dns: ip.dns,
            bitrate,
            mtu,
        })
    } else {
        None
    };
    Ok(WifiDetails {
        adapter: Adapter::of_net_interface(&device.interface),
        addressing,
    })
}

impl Source for Wifi {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = NAME;
}

impl LiveSource for Wifi {
    fn events(_key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        device_or_absent(nm::SERVICE, || nm_readings(NAME, read).boxed())
    }
}

impl Source for WifiDetails {
    type Error = Infallible;
    type Key = ();

    const NAME: &'static str = "wifi details";
}

impl LiveSource for WifiDetails {
    fn events(_key: ()) -> impl Stream<Item = Delivery<Self>> + Send {
        device_or_absent(nm::SERVICE, || {
            nm_readings(Self::NAME, read_details).boxed()
        })
    }
}
