use std::net::IpAddr;

use futures::{StreamExt, TryStreamExt, stream, try_join};
use zbus::{Connection, zvariant::OwnedObjectPath};

use super::{
    ActiveConnectionProxy, AddressAssignment, AddressBlock, BUS_OBJECT_READS_IN_FLIGHT, DeviceKind,
    DeviceProxy, LinkPhase, RouterGateway, assigned_object_path, devices, ip,
    network_manager_proxy, uncached,
};
use crate::system_protocol_clients::dbus_desktop_buses::BusReadError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    Full,
    Portal,
    Limited,
    Offline,
}

impl TryFrom<u32> for Connectivity {
    type Error = u32;

    fn try_from(state_code: u32) -> Result<Self, Self::Error> {
        match state_code {
            4 => Ok(Self::Full),
            3 => Ok(Self::Limited),
            2 => Ok(Self::Portal),
            1 => Ok(Self::Offline),
            unknown => Err(unknown),
        }
    }
}

impl Connectivity {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Portal => "portal login",
            Self::Limited => "limited",
            Self::Offline => "none",
        }
    }
}

pub struct Uplink {
    pub activated_interfaces: Vec<String>,
    pub metering: Metering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Metering {
    Metered,
    #[default]
    Unmetered,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeviceGateway {
    pub interface: String,
    pub gateway: IpAddr,
}

const METERED_YES: u32 = 1;
const METERED_GUESS_YES: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveConnectionUplink {
    pub connection_name: String,
    pub interface: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UplinkIdentity {
    Named(ActiveConnectionUplink),
    Unnamed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectedScope {
    Local,
    Site,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerState {
    Asleep,
    Disconnected,
    Disconnecting,
    Connecting,
    Connected(ConnectedScope),
    Unknown,
}

impl From<u32> for ManagerState {
    fn from(state_code: u32) -> Self {
        match state_code {
            10 => Self::Asleep,
            20 => Self::Disconnected,
            30 => Self::Disconnecting,
            40 => Self::Connecting,
            50 => Self::Connected(ConnectedScope::Local),
            60 => Self::Connected(ConnectedScope::Site),
            70 => Self::Connected(ConnectedScope::Global),
            _ => Self::Unknown,
        }
    }
}

pub async fn networking_enabled(connection: &Connection) -> zbus::Result<bool> {
    network_manager_proxy(connection)
        .await?
        .networking_enabled()
        .await
}

pub async fn manager_state(connection: &Connection) -> zbus::Result<ManagerState> {
    Ok(ManagerState::from(
        network_manager_proxy(connection).await?.state().await?,
    ))
}

pub async fn checked_connectivity(
    connection: &Connection,
) -> zbus::Result<Result<Connectivity, u32>> {
    Ok(Connectivity::try_from(
        network_manager_proxy(connection)
            .await?
            .connectivity()
            .await?,
    ))
}

pub async fn primary_uplink(connection: &Connection) -> zbus::Result<UplinkIdentity> {
    let primary_connection = network_manager_proxy(connection)
        .await?
        .primary_connection()
        .await?;
    uplink_identity(connection, primary_connection).await
}

pub async fn activating_uplink(connection: &Connection) -> zbus::Result<UplinkIdentity> {
    let activating_connection = network_manager_proxy(connection)
        .await?
        .activating_connection()
        .await?;
    uplink_identity(connection, activating_connection).await
}

async fn uplink_identity(
    connection: &Connection,
    active_connection: OwnedObjectPath,
) -> zbus::Result<UplinkIdentity> {
    let Some(active_connection) = assigned_object_path(active_connection) else {
        return Ok(UplinkIdentity::Unnamed);
    };
    let active: ActiveConnectionProxy = uncached(connection, active_connection).await?;
    let (connection_name, device_paths) = try_join!(active.id(), active.devices())?;
    let interface = match device_paths.into_iter().next() {
        Some(device_path) => {
            let device: DeviceProxy = uncached(connection, device_path).await?;
            device.interface().await?
        }
        None => String::new(),
    };
    Ok(UplinkIdentity::Named(ActiveConnectionUplink {
        connection_name,
        interface,
    }))
}

pub async fn uplink(connection: Connection) -> Result<Uplink, BusReadError> {
    let manager = network_manager_proxy(&connection).await?;
    let (paths, metered) = try_join!(manager.get_devices(), manager.metered())?;
    let interfaces = stream::iter(paths)
        .map(|path| device_interface(&connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .try_collect::<Vec<_>>()
        .await?;
    Ok(Uplink {
        activated_interfaces: interfaces
            .into_iter()
            .filter(|interface| interface.link_state == LinkPhase::Up && !interface.name.is_empty())
            .map(|interface| interface.name)
            .collect(),
        metering: match metered {
            METERED_YES | METERED_GUESS_YES => Metering::Metered,
            _ => Metering::Unmetered,
        },
    })
}

pub async fn manager_connectivity(connection: Connection) -> Result<Connectivity, BusReadError> {
    let state_code = network_manager_proxy(&connection)
        .await?
        .connectivity()
        .await?;
    Connectivity::try_from(state_code).map_err(|_| BusReadError::Absent)
}

pub async fn device_gateways(connection: Connection) -> Result<Vec<DeviceGateway>, BusReadError> {
    let routed_devices = devices(&connection).await?.into_iter().filter(|device| {
        device.link_state == LinkPhase::Up
            && matches!(device.kind, DeviceKind::Ethernet | DeviceKind::Wifi)
    });
    let gateways_per_device = stream::iter(routed_devices)
        .map(|device| {
            let connection = &connection;
            async move {
                let addressing = ip(connection, &device).await?;
                Ok::<_, zbus::Error>(
                    [addressing.v4, addressing.v6]
                        .into_iter()
                        .filter_map(|assignment| {
                            let AddressAssignment::Assigned(AddressBlock {
                                gateway: RouterGateway::Routed(gateway),
                                ..
                            }) = assignment
                            else {
                                return None;
                            };
                            gateway
                                .parse()
                                .inspect_err(|parse_failure| {
                                    tracing::warn!(
                                        "network manager: invalid gateway {gateway:?} on {}: \
                                         {parse_failure}",
                                        device.interface
                                    );
                                })
                                .ok()
                        })
                        .map(|gateway| DeviceGateway {
                            interface: device.interface.clone(),
                            gateway,
                        })
                        .collect::<Vec<_>>(),
                )
            }
        })
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .try_collect::<Vec<_>>()
        .await?;
    Ok(gateways_per_device.into_iter().flatten().collect())
}

struct DeviceInterface {
    link_state: LinkPhase,
    name: String,
}

async fn device_interface(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<DeviceInterface> {
    let device: DeviceProxy = uncached(connection, path).await?;
    let (state, name) = try_join!(device.state(), device.ip_interface())?;
    Ok(DeviceInterface {
        link_state: LinkPhase::from(state),
        name,
    })
}

pub async fn connection_id(
    connection: &Connection,
    active_connection: OwnedObjectPath,
) -> zbus::Result<String> {
    let active: ActiveConnectionProxy = uncached(connection, active_connection).await?;
    active.id().await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioPower {
    On,
    Off,
}

impl RadioPower {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            RadioPower::On => "on",
            RadioPower::Off => "off",
        }
    }
}

pub async fn wireless_radio_power(connection: &Connection) -> zbus::Result<RadioPower> {
    if network_manager_proxy(connection)
        .await?
        .wireless_enabled()
        .await?
    {
        Ok(RadioPower::On)
    } else {
        Ok(RadioPower::Off)
    }
}
