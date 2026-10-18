use std::future::ready;

use futures::{
    Stream, StreamExt,
    stream::{self, select},
    try_join,
};
use uom::si::f64::{Frequency, InformationRate};
use zbus::{Connection, zvariant::OwnedObjectPath};

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    bus::DesktopBuses,
    hardware::Adapter,
    net_interfaces::{ReadingError, device_or_absent, device_readings, nm, nm_readings},
    stream_operators::{read_once_then_per_burst, resubscribing_until_failure},
};

pub const NAME: &str = "wifi";

#[derive(Debug, Clone)]
pub struct Link {
    access_point: OwnedObjectPath,
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
            let ap = nm::access_point(&connection, path.clone()).await?;
            Some(Link {
                access_point: path,
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

impl ReadingSource for Wifi {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = NAME;
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(system.clone(), nm::SERVICE, move || {
            let system = system.clone();
            resubscribing_until_failure(move || readings_while_access_point_holds(system.clone()))
        })
    }
}

async fn readings_while_access_point_holds(
    connection: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Option<Wifi>>>> {
    let watched_access_point = read(connection.clone())
        .await
        .ok()
        .and_then(|wifi| wifi.access_point());
    let device_changes = nm::changes(
        &connection,
        &[
            nm::MANAGER_PROPERTIES,
            nm::DEVICE_PROPERTIES,
            nm::WIRELESS_PROPERTIES,
        ],
    )
    .await?;
    let strength_changes = match watched_access_point.clone() {
        Some(path) => Some(nm::access_point_changes(&connection, path).await?),
        None => None,
    };
    let strength_changes = stream::iter(strength_changes).flatten();
    let readings = read_once_then_per_burst(select(device_changes, strength_changes), move || {
        read(connection.clone())
    })
    .take_while(move |reading| {
        ready(
            reading
                .as_ref()
                .map_or(true, |wifi| wifi.access_point() == watched_access_point),
        )
    });
    Ok(device_readings(readings))
}

impl Wifi {
    fn access_point(&self) -> Option<OwnedObjectPath> {
        self.link.as_ref().map(|link| link.access_point.clone())
    }
}

impl ReadingSource for WifiDetails {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "wifi details";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PushedLive;

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(system.clone(), nm::SERVICE, move || {
            nm_readings(
                system.clone(),
                &[
                    nm::MANAGER_PROPERTIES,
                    nm::DEVICE_PROPERTIES,
                    nm::WIRELESS_PROPERTIES,
                    nm::IP4_CONFIG_PROPERTIES,
                    nm::IP6_CONFIG_PROPERTIES,
                ],
                read_details,
            )
        })
    }
}
