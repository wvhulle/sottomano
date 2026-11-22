use std::ops::RangeInclusive;

use futures::{StreamExt, TryStreamExt, stream, try_join};
use uom::si::{f64::InformationRate, information_rate::megabit_per_second};
use zbus::{Connection, zvariant::OwnedObjectPath};

use super::{BUS_OBJECT_READS_IN_FLIGHT, DeviceProxy, WiredProxy, network_manager_proxy, uncached};
use crate::system_protocol_clients::dbus_desktop_buses::BusReadError;

const STATE_ACTIVATED: u32 = 100;
const STATES_ACTIVATING: RangeInclusive<u32> = 40..=90;
const STATE_DEACTIVATING: u32 = 110;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Ethernet,
    Wifi,
    Modem,
    Other,
}

impl From<u32> for DeviceKind {
    fn from(code: u32) -> Self {
        match code {
            1 => Self::Ethernet,
            2 => Self::Wifi,
            8 => Self::Modem,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkPhase {
    Up,
    Switching,
    Down,
}

impl From<u32> for LinkPhase {
    fn from(state_code: u32) -> Self {
        if state_code == STATE_ACTIVATED {
            LinkPhase::Up
        } else if STATES_ACTIVATING.contains(&state_code) || state_code == STATE_DEACTIVATING {
            LinkPhase::Switching
        } else {
            LinkPhase::Down
        }
    }
}

impl LinkPhase {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            LinkPhase::Up => "connected",
            LinkPhase::Switching => "switching",
            LinkPhase::Down => "disconnected",
        }
    }
}

pub struct DeviceInfo {
    pub interface: String,
    pub kind: DeviceKind,
    pub link_state: LinkPhase,
    pub path: OwnedObjectPath,
    pub proxy: DeviceProxy<'static>,
}

pub async fn devices(connection: &Connection) -> zbus::Result<Vec<DeviceInfo>> {
    let paths = network_manager_proxy(connection)
        .await?
        .get_devices()
        .await?;
    stream::iter(paths)
        .map(|path| device_info(connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .try_collect::<Vec<_>>()
        .await
}

pub async fn devices_of_kind(
    connection: &Connection,
    kind: DeviceKind,
) -> Result<Vec<DeviceInfo>, BusReadError> {
    let mut matching_devices: Vec<DeviceInfo> = devices(connection)
        .await?
        .into_iter()
        .filter(|device| device.kind == kind)
        .collect();
    if matching_devices.is_empty() {
        return Err(BusReadError::Absent);
    }
    matching_devices.sort_by(|left, right| left.interface.cmp(&right.interface));
    Ok(matching_devices)
}

pub async fn modem_device_at(
    connection: &Connection,
    modem_path: &str,
) -> zbus::Result<Option<DeviceInfo>> {
    let modem_devices = devices(connection)
        .await?
        .into_iter()
        .filter(|device| device.kind == DeviceKind::Modem);
    for modem_device in modem_devices {
        if modem_device.proxy.udi().await? == modem_path {
            return Ok(Some(modem_device));
        }
    }
    Ok(None)
}

async fn device_info(connection: &Connection, path: OwnedObjectPath) -> zbus::Result<DeviceInfo> {
    let proxy: DeviceProxy = uncached(connection, path.clone()).await?;
    let (kind, interface, state) =
        try_join!(proxy.device_type(), proxy.interface(), proxy.state())?;
    Ok(DeviceInfo {
        kind: DeviceKind::from(kind),
        interface,
        link_state: LinkPhase::from(state),
        path,
        proxy,
    })
}

pub async fn active_connection(device: &DeviceInfo) -> zbus::Result<OwnedObjectPath> {
    device.proxy.active_connection().await
}

pub async fn hw_address(device: &DeviceInfo) -> zbus::Result<String> {
    device.proxy.hw_address().await
}

pub async fn mtu(device: &DeviceInfo) -> zbus::Result<u32> {
    device.proxy.mtu().await
}

pub async fn wired_speed(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<InformationRate> {
    let wired: WiredProxy = uncached(connection, device.path.clone()).await?;
    let megabits_per_second = wired.speed().await?;
    Ok(InformationRate::new::<megabit_per_second>(f64::from(
        megabits_per_second,
    )))
}
