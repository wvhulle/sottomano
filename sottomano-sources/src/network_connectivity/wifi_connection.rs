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
    host_machine_identity::udev_hardware_identity::Adapter,
    network_connectivity::{
        ReadingError, device_or_absent, device_readings, network_manager_proxies, nm_readings,
    },
    stream_operators::{read_once_then_per_burst, resubscribing_until_failure},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
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
    pub ipv4: Option<network_manager_proxies::AddressBlock>,
    pub ipv6: Option<network_manager_proxies::AddressBlock>,
    pub dns: Vec<String>,
    pub bitrate: InformationRate,
    pub mtu: u32,
}

#[derive(Debug, Clone)]
pub struct WifiDetails {
    pub adapter: Option<Adapter>,
    pub addressing: Option<Addressing>,
}

async fn wifi_device(
    connection: &Connection,
) -> Result<network_manager_proxies::DeviceInfo, ReadingError> {
    network_manager_proxies::devices(connection)
        .await?
        .into_iter()
        .find(|device| device.kind == network_manager_proxies::DeviceKind::Wifi)
        .ok_or(ReadingError::Absent)
}

async fn read(connection: Connection) -> Result<Wifi, ReadingError> {
    let device = wifi_device(&connection).await?;
    let ap_path = if device.connected {
        Some(network_manager_proxies::active_access_point(&connection, &device).await?)
    } else {
        None
    };
    let link = match ap_path.filter(|path| !network_manager_proxies::is_unset(path)) {
        Some(path) => {
            let ap = network_manager_proxies::access_point(&connection, path.clone()).await?;
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
            network_manager_proxies::ip(&connection, &device),
            network_manager_proxies::hw_address(&device),
            network_manager_proxies::wireless_rate(&connection, &device),
            network_manager_proxies::mtu(&device)
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
        device_or_absent(
            system.clone(),
            network_manager_proxies::SERVICE,
            move || {
                let system = system.clone();
                resubscribing_until_failure(move || {
                    readings_while_access_point_holds(system.clone())
                })
            },
        )
    }
}

async fn readings_while_access_point_holds(
    connection: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Option<Wifi>>>> {
    let watched_access_point = read(connection.clone())
        .await
        .ok()
        .and_then(|wifi| wifi.access_point());
    let device_changes = network_manager_proxies::changes(
        &connection,
        &[
            network_manager_proxies::MANAGER_PROPERTIES,
            network_manager_proxies::DEVICE_PROPERTIES,
            network_manager_proxies::WIRELESS_PROPERTIES,
        ],
    )
    .await?;
    let strength_changes = match watched_access_point.clone() {
        Some(path) => Some(network_manager_proxies::access_point_changes(&connection, path).await?),
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
        device_or_absent(
            system.clone(),
            network_manager_proxies::SERVICE,
            move || {
                nm_readings(
                    system.clone(),
                    &[
                        network_manager_proxies::MANAGER_PROPERTIES,
                        network_manager_proxies::DEVICE_PROPERTIES,
                        network_manager_proxies::WIRELESS_PROPERTIES,
                        network_manager_proxies::IP4_CONFIG_PROPERTIES,
                        network_manager_proxies::IP6_CONFIG_PROPERTIES,
                    ],
                    read_details,
                )
            },
        )
    }
}
