use futures::{Stream, future::try_join_all, try_join};
use uom::si::f64::{Frequency, InformationRate};
use zbus::Connection;

use crate::{
    SnapshotDelivery, SnapshotSource,
    host_machine_identity::udev_hardware_identity::Adapter,
    network_connectivity::{
        ReadingError, device_or_absent, interface_bus_attachment::InterfaceAttachment,
        network_manager_proxies, nm_readings, sysfs_interface_faults::InterfaceFaultCounters,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

#[derive(Debug, Clone)]
pub struct Link {
    pub ssid: Option<String>,
    pub signal: i64,
    pub frequency: Frequency,
    pub security: String,
}

#[derive(Debug, Clone)]
pub struct WifiAdapter {
    pub device: String,
    pub attachment: InterfaceAttachment,
    pub link_state: network_manager_proxies::DeviceLinkState,
    pub link: Option<Link>,
}

#[derive(Debug, Clone)]
pub struct Wifi {
    pub radio_power: network_manager_proxies::RadioPower,
    pub adapters: Vec<WifiAdapter>,
}

#[derive(Debug, Clone)]
pub struct Addressing {
    pub mac: String,
    pub ipv4: Option<network_manager_proxies::AddressBlock>,
    pub ipv6: Option<network_manager_proxies::AddressBlock>,
    pub dns: Vec<String>,
    pub bitrate: InformationRate,
    pub mtu: u32,
    pub fault_counters: InterfaceFaultCounters,
}

#[derive(Debug, Clone)]
pub struct WifiAdapterDetails {
    pub device: String,
    pub adapter: Option<Adapter>,
    pub addressing: Option<Addressing>,
}

#[derive(Debug, Clone)]
pub struct WifiDetails {
    pub adapters: Vec<WifiAdapterDetails>,
}

async fn wifi_devices(
    connection: &Connection,
) -> Result<Vec<network_manager_proxies::DeviceInfo>, ReadingError> {
    network_manager_proxies::devices_of_kind(connection, network_manager_proxies::DeviceKind::Wifi)
        .await
}

async fn adapter_link(
    connection: &Connection,
    device: &network_manager_proxies::DeviceInfo,
) -> zbus::Result<Option<Link>> {
    let ap_path = if device.link_state == network_manager_proxies::DeviceLinkState::Connected {
        Some(network_manager_proxies::active_access_point(connection, device).await?)
    } else {
        None
    };
    match ap_path.and_then(network_manager_proxies::assigned_object_path) {
        Some(path) => {
            let ap = network_manager_proxies::access_point(connection, path).await?;
            Ok(Some(Link {
                ssid: ap.ssid,
                signal: ap.signal,
                frequency: ap.frequency,
                security: ap.security,
            }))
        }
        None => Ok(None),
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

async fn read(connection: Connection) -> Result<Wifi, ReadingError> {
    let (devices, radio_power) = try_join!(wifi_devices(&connection), async {
        Ok(network_manager_proxies::wireless_radio_power(&connection).await?)
    })?;
    let adapters = try_join_all(
        devices
            .into_iter()
            .map(|device| wifi_adapter(&connection, device)),
    )
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
    let addressing = if device.link_state == network_manager_proxies::DeviceLinkState::Connected {
        let (ip, mac, bitrate, mtu) = try_join!(
            network_manager_proxies::ip(connection, &device),
            network_manager_proxies::hw_address(&device),
            network_manager_proxies::wireless_rate(connection, &device),
            network_manager_proxies::mtu(&device)
        )?;
        Some(Addressing {
            mac,
            ipv4: ip.v4,
            ipv6: ip.v6,
            dns: ip.dns,
            bitrate,
            mtu,
            fault_counters: InterfaceFaultCounters::from(device.interface.as_str()),
        })
    } else {
        None
    };
    Ok(WifiAdapterDetails {
        adapter: Adapter::of_net_interface(&device.interface),
        device: device.interface,
        addressing,
    })
}

async fn read_details(connection: Connection) -> Result<WifiDetails, ReadingError> {
    let devices = wifi_devices(&connection).await?;
    let adapters = try_join_all(
        devices
            .into_iter()
            .map(|device| adapter_details(&connection, device)),
    )
    .await?;
    Ok(WifiDetails { adapters })
}

impl SnapshotSource for Wifi {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Wi-Fi";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
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
                        network_manager_proxies::ACCESS_POINT_PROPERTIES,
                    ],
                    read,
                )
            },
        )
    }
}

impl SnapshotSource for WifiDetails {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "Wi-Fi details";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
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
