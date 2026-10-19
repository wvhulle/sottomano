use futures::{Stream, future::try_join_all, try_join};
use uom::si::f64::InformationRate;
use zbus::Connection;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery,
    host_machine_identity::udev_hardware_identity::Adapter,
    network_connectivity::{ReadingError, device_or_absent, network_manager_proxies, nm_readings},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

pub const NAME: &str = "ethernet";

#[derive(Debug, Clone)]
pub struct Link {
    pub connection: Option<String>,
    pub mac: String,
    pub ipv4: Option<network_manager_proxies::AddressBlock>,
    pub ipv6: Option<network_manager_proxies::AddressBlock>,
    pub dns: Vec<String>,
    pub speed: InformationRate,
    pub mtu: u32,
}

#[derive(Debug, Clone)]
pub struct PortState {
    pub name: String,
    pub connected: bool,
}

#[derive(Debug, Clone)]
pub struct Ethernet {
    pub ports: Vec<PortState>,
}

impl Ethernet {
    #[must_use]
    pub fn connected_count(&self) -> usize {
        self.ports.iter().filter(|port| port.connected).count()
    }
}

#[derive(Debug, Clone)]
pub struct Port {
    pub name: String,
    pub adapter: Option<Adapter>,
    pub link: Option<Link>,
}

#[derive(Debug, Clone)]
pub struct EthernetDetails {
    pub ports: Vec<Port>,
}

async fn port(
    connection: &Connection,
    device: network_manager_proxies::DeviceInfo,
) -> zbus::Result<Port> {
    let adapter = Adapter::of_net_interface(&device.interface);
    let link = if device.connected {
        Some(link(connection, &device).await?)
    } else {
        None
    };
    Ok(Port {
        adapter,
        name: device.interface,
        link,
    })
}

async fn link(
    connection: &Connection,
    device: &network_manager_proxies::DeviceInfo,
) -> zbus::Result<Link> {
    let (ip, active_connection, mac, speed, mtu) = try_join!(
        network_manager_proxies::ip(connection, device),
        network_manager_proxies::active_connection(device),
        network_manager_proxies::hw_address(device),
        network_manager_proxies::wired_speed(connection, device),
        network_manager_proxies::mtu(device)
    )?;
    let profile = if network_manager_proxies::is_unset(&active_connection) {
        None
    } else {
        Some(network_manager_proxies::connection_id(connection, active_connection).await?)
    };
    Ok(Link {
        connection: profile,
        mac,
        ipv4: ip.v4,
        ipv6: ip.v6,
        dns: ip.dns,
        speed,
        mtu,
    })
}

async fn ethernet_devices(
    connection: &Connection,
) -> Result<Vec<network_manager_proxies::DeviceInfo>, ReadingError> {
    let devices: Vec<network_manager_proxies::DeviceInfo> =
        network_manager_proxies::devices(connection)
            .await?
            .into_iter()
            .filter(|device| device.kind == network_manager_proxies::DeviceKind::Ethernet)
            .collect();
    if devices.is_empty() {
        return Err(ReadingError::Absent);
    }
    Ok(devices)
}

async fn read(connection: Connection) -> Result<Ethernet, ReadingError> {
    Ok(Ethernet {
        ports: ethernet_devices(&connection)
            .await?
            .into_iter()
            .map(|device| PortState {
                connected: device.connected,
                name: device.interface,
            })
            .collect(),
    })
}

async fn read_details(connection: Connection) -> Result<EthernetDetails, ReadingError> {
    let devices = ethernet_devices(&connection).await?;
    let ports = try_join_all(devices.into_iter().map(|device| port(&connection, device))).await?;
    Ok(EthernetDetails { ports })
}

impl ReadingSource for Ethernet {
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
                nm_readings(
                    system.clone(),
                    network_manager_proxies::UPLINK_PROPERTIES,
                    read,
                )
            },
        )
    }
}

impl ReadingSource for EthernetDetails {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "ethernet details";
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
                        network_manager_proxies::WIRED_PROPERTIES,
                        network_manager_proxies::IP4_CONFIG_PROPERTIES,
                        network_manager_proxies::IP6_CONFIG_PROPERTIES,
                    ],
                    read_details,
                )
            },
        )
    }
}
