use futures::{Stream, StreamExt, TryStreamExt, stream, try_join};
use uom::si::f64::{Frequency, InformationRate};
use zbus::Connection;

use crate::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::{
        device_or_absent,
        interface_bus_attachment::{AdapterIdentity, InterfaceAttachment},
        network_manager_proxies, nm_samples,
        sysfs_interface_faults::InterfaceFaultCounters,
    },
    system_protocol_clients::dbus_desktop_buses::{BusReadError, DesktopBuses},
};

#[derive(Debug, Clone)]
pub struct Link {
    pub ssid: network_manager_proxies::NetworkName,
    pub signal: i64,
    pub frequency: Frequency,
    pub security: String,
}

#[derive(Debug, Clone)]
pub struct WifiAdapter {
    pub device: String,
    pub attachment: InterfaceAttachment,
    pub link_state: network_manager_proxies::LinkPhase,
    pub link: AccessPointAssociation,
}

#[derive(Debug, Clone)]
pub enum AccessPointAssociation {
    Associated(Link),
    Unassociated,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    pub radio_power: network_manager_proxies::RadioPower,
    pub adapters: Vec<WifiAdapter>,
}

#[derive(Debug, Clone)]
pub struct Addressing {
    pub mac: String,
    pub ipv4: network_manager_proxies::AddressAssignment,
    pub ipv6: network_manager_proxies::AddressAssignment,
    pub dns: Vec<String>,
    pub bitrate: InformationRate,
    pub mtu: u32,
    pub fault_counters: InterfaceFaultCounters,
}

#[derive(Debug, Clone)]
pub struct WifiAdapterDetails {
    pub device: String,
    pub adapter: AdapterIdentity,
    pub addressing: AdapterAddressing,
}

#[derive(Debug, Clone)]
pub enum AdapterAddressing {
    Addressed(Box<Addressing>),
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct WifiDetails {
    pub adapters: Vec<WifiAdapterDetails>,
}

async fn wifi_devices(
    connection: &Connection,
) -> Result<Vec<network_manager_proxies::DeviceInfo>, BusReadError> {
    network_manager_proxies::devices_of_kind(connection, network_manager_proxies::DeviceKind::Wifi)
        .await
}

async fn adapter_link(
    connection: &Connection,
    device: &network_manager_proxies::DeviceInfo,
) -> zbus::Result<AccessPointAssociation> {
    if device.link_state != network_manager_proxies::LinkPhase::Up {
        return Ok(AccessPointAssociation::Unassociated);
    }
    let ap_path = network_manager_proxies::active_access_point(connection, device).await?;
    match network_manager_proxies::assigned_object_path(ap_path) {
        Some(path) => {
            let ap = network_manager_proxies::access_point(connection, path).await?;
            Ok(AccessPointAssociation::Associated(Link {
                ssid: ap.ssid,
                signal: ap.signal,
                frequency: ap.frequency,
                security: ap.security,
            }))
        }
        None => Ok(AccessPointAssociation::Unassociated),
    }
}

async fn wifi_adapter(
    connection: &Connection,
    device: network_manager_proxies::DeviceInfo,
) -> zbus::Result<WifiAdapter> {
    let link = adapter_link(connection, &device).await?;
    Ok(WifiAdapter {
        attachment: InterfaceAttachment::from(device.interface.as_str()),
        link_state: device.link_state,
        device: device.interface,
        link,
    })
}

async fn read(connection: Connection) -> Result<Wifi, BusReadError> {
    let (devices, radio_power) = try_join!(wifi_devices(&connection), async {
        Ok(network_manager_proxies::wireless_radio_power(&connection).await?)
    })?;
    let adapters = stream::iter(devices)
        .map(|device| wifi_adapter(&connection, device))
        .buffered(network_manager_proxies::BUS_OBJECT_READS_IN_FLIGHT)
        .try_collect::<Vec<_>>()
        .await?;
    Ok(Wifi {
        radio_power,
        adapters,
    })
}

async fn adapter_details(
    connection: &Connection,
    device: network_manager_proxies::DeviceInfo,
) -> zbus::Result<WifiAdapterDetails> {
    let addressing = if device.link_state == network_manager_proxies::LinkPhase::Up {
        let (ip, mac, bitrate, mtu) = try_join!(
            network_manager_proxies::ip(connection, &device),
            network_manager_proxies::hw_address(&device),
            network_manager_proxies::wireless_rate(connection, &device),
            network_manager_proxies::mtu(&device)
        )?;
        AdapterAddressing::Addressed(Box::new(Addressing {
            mac,
            ipv4: ip.v4,
            ipv6: ip.v6,
            dns: ip.dns,
            bitrate,
            mtu,
            fault_counters: InterfaceFaultCounters::from(device.interface.as_str()),
        }))
    } else {
        AdapterAddressing::Disconnected
    };
    Ok(WifiAdapterDetails {
        adapter: AdapterIdentity::net_interface(&device.interface),
        device: device.interface,
        addressing,
    })
}

async fn read_details(connection: Connection) -> Result<WifiDetails, BusReadError> {
    let devices = wifi_devices(&connection).await?;
    let adapters = stream::iter(devices)
        .map(|device| adapter_details(&connection, device))
        .buffered(network_manager_proxies::BUS_OBJECT_READS_IN_FLIGHT)
        .try_collect::<Vec<_>>()
        .await?;
    Ok(WifiDetails { adapters })
}

impl SnapshotSource for Wifi {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Wi-Fi";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(
            system.clone(),
            network_manager_proxies::SERVICE,
            move || {
                nm_samples(
                    system.clone(),
                    &[
                        network_manager_proxies::MANAGER_PROPERTIES,
                        network_manager_proxies::DEVICE_PROPERTIES,
                        network_manager_proxies::WIRELESS_PROPERTIES,
                        network_manager_proxies::ACCESS_POINT_PROPERTIES,
                    ],
                    read,
                )
            },
        )
    }
}

impl SnapshotSource for WifiDetails {
    type Error = BusReadError;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Wi-Fi details";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(
            system.clone(),
            network_manager_proxies::SERVICE,
            move || {
                nm_samples(
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
