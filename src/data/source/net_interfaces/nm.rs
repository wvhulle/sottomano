use std::{
    collections::HashMap,
    net::{Ipv4Addr, Ipv6Addr},
};

use iced::futures::{Stream, StreamExt, future::try_join_all, try_join};
use uom::si::{
    f64::{Frequency, InformationRate},
    frequency::megahertz,
    information_rate::{kilobit_per_second, megabit_per_second},
};
use zbus::{
    Connection, MatchRule, MessageStream,
    message::Type,
    proxy,
    proxy::{Builder, CacheProperties, Defaults},
    zvariant::{OwnedObjectPath, OwnedValue},
};

use crate::data::empty_text::non_empty;

pub const SERVICE: &str = "org.freedesktop.NetworkManager";

const STATE_ACTIVATED: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Ethernet,
    Wifi,
    Other,
}

impl From<u32> for DeviceKind {
    fn from(code: u32) -> Self {
        match code {
            1 => Self::Ethernet,
            2 => Self::Wifi,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Connectivity {
    Full,
    Limited,
    None,
    #[default]
    Unknown,
}

impl From<u32> for Connectivity {
    fn from(code: u32) -> Self {
        match code {
            4 => Self::Full,
            2 | 3 => Self::Limited,
            1 => Self::None,
            _ => Self::Unknown,
        }
    }
}

impl Connectivity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Limited => "limited",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }
}

#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait Manager {
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Device {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn ip4_config(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn ip6_config(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn active_connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn hw_address(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn mtu(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn ip_interface(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wired",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wired {
    #[zbus(property)]
    fn speed(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wireless {
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn bitrate(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;
    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;
    #[zbus(property)]
    fn frequency(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn wpa_flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait ActiveConnection {
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.IP4Config",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Ip4Config {
    #[zbus(property)]
    fn address_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
    #[zbus(property)]
    fn gateway(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn nameserver_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.IP6Config",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Ip6Config {
    #[zbus(property)]
    fn address_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
    #[zbus(property)]
    fn gateway(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn nameservers(&self) -> zbus::Result<Vec<Vec<u8>>>;
}

async fn uncached<T>(connection: &Connection, path: OwnedObjectPath) -> zbus::Result<T>
where
    T: From<zbus::Proxy<'static>> + Defaults,
{
    Builder::new(connection)
        .path(path)?
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

async fn manager(connection: &Connection) -> zbus::Result<ManagerProxy<'static>> {
    Builder::new(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

pub struct DeviceInfo {
    pub interface: String,
    pub kind: DeviceKind,
    pub connected: bool,
    path: OwnedObjectPath,
    proxy: DeviceProxy<'static>,
}

pub async fn devices(connection: &Connection) -> zbus::Result<Vec<DeviceInfo>> {
    let paths = manager(connection).await?.get_devices().await?;
    try_join_all(paths.into_iter().map(|path| device_info(connection, path))).await
}

async fn device_info(connection: &Connection, path: OwnedObjectPath) -> zbus::Result<DeviceInfo> {
    let proxy: DeviceProxy = uncached(connection, path.clone()).await?;
    let (kind, interface, state) =
        try_join!(proxy.device_type(), proxy.interface(), proxy.state())?;
    Ok(DeviceInfo {
        kind: DeviceKind::from(kind),
        interface,
        connected: state == STATE_ACTIVATED,
        path,
        proxy,
    })
}

pub struct Uplink {
    pub activated_interfaces: Vec<String>,
    pub connectivity: Connectivity,
}

pub async fn uplink(connection: Connection) -> zbus::Result<Option<Uplink>> {
    let manager = manager(&connection).await?;
    let (paths, connectivity) = try_join!(manager.get_devices(), manager.connectivity())?;
    let interfaces = try_join_all(
        paths
            .into_iter()
            .map(|path| activated_ip_interface(&connection, path)),
    )
    .await?;
    Ok(Some(Uplink {
        activated_interfaces: interfaces.into_iter().flatten().collect(),
        connectivity: Connectivity::from(connectivity),
    }))
}

async fn activated_ip_interface(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<Option<String>> {
    let device: DeviceProxy = uncached(connection, path).await?;
    let (state, interface) = try_join!(device.state(), device.ip_interface())?;
    Ok((state == STATE_ACTIVATED)
        .then_some(interface)
        .and_then(non_empty))
}

pub async fn connection_id(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<Option<String>> {
    let path = device.proxy.active_connection().await?;
    if is_unset(&path) {
        return Ok(None);
    }
    let active: ActiveConnectionProxy = uncached(connection, path).await?;
    Ok(Some(active.id().await?))
}

fn is_unset(path: &OwnedObjectPath) -> bool {
    path.as_str() == "/"
}

#[derive(Debug, Clone)]
pub struct AddressBlock {
    pub address: String,
    pub mask: String,
    pub gateway: Option<String>,
}

pub struct IpInfo {
    pub v4: Option<AddressBlock>,
    pub v6: Option<AddressBlock>,
    pub dns: Vec<String>,
}

pub struct FamilyConfig {
    block: Option<AddressBlock>,
    dns: Vec<String>,
}

impl FamilyConfig {
    fn unconfigured() -> Self {
        Self {
            block: None,
            dns: Vec::new(),
        }
    }
}

pub async fn ip(connection: &Connection, device: &DeviceInfo) -> zbus::Result<IpInfo> {
    let (v4_path, v6_path) = try_join!(device.proxy.ip4_config(), device.proxy.ip6_config())?;
    let (v4, v6) = try_join!(
        ip4_config(connection, v4_path),
        ip6_config(connection, v6_path)
    )?;
    Ok(IpInfo {
        v4: v4.block,
        v6: v6.block,
        dns: v4.dns.into_iter().chain(v6.dns).collect(),
    })
}

fn first_address(entries: &[HashMap<String, OwnedValue>]) -> Option<(String, u32)> {
    let global = entries.iter().find(|entry| {
        entry
            .get("address")
            .and_then(|value| value.downcast_ref::<String>().ok())
            .is_some_and(|address| !address.starts_with("fe80"))
    });
    let entry = global.or_else(|| entries.first())?;
    let address = entry.get("address")?.downcast_ref::<String>().ok()?;
    let prefix = entry.get("prefix")?.downcast_ref::<u32>().ok()?;
    Some((address, prefix))
}

async fn ip4_config(connection: &Connection, path: OwnedObjectPath) -> zbus::Result<FamilyConfig> {
    if is_unset(&path) {
        return Ok(FamilyConfig::unconfigured());
    }
    let config: Ip4ConfigProxy = uncached(connection, path).await?;
    let (servers, addresses, gateway) = try_join!(
        config.nameserver_data(),
        config.address_data(),
        config.gateway()
    )?;
    Ok(FamilyConfig {
        block: first_address(&addresses).map(|(address, prefix)| AddressBlock {
            address,
            mask: prefix_to_netmask(prefix),
            gateway: non_empty(gateway),
        }),
        dns: servers
            .into_iter()
            .filter_map(|entry| entry.get("address")?.downcast_ref::<String>().ok())
            .collect(),
    })
}

async fn ip6_config(connection: &Connection, path: OwnedObjectPath) -> zbus::Result<FamilyConfig> {
    if is_unset(&path) {
        return Ok(FamilyConfig::unconfigured());
    }
    let config: Ip6ConfigProxy = uncached(connection, path).await?;
    let (servers, addresses, gateway) = try_join!(
        config.nameservers(),
        config.address_data(),
        config.gateway()
    )?;
    Ok(FamilyConfig {
        block: first_address(&addresses).map(|(address, prefix)| AddressBlock {
            address,
            mask: format!("/{prefix}"),
            gateway: non_empty(gateway),
        }),
        dns: servers
            .into_iter()
            .filter_map(|bytes| {
                let raw = <[u8; 16]>::try_from(bytes.as_slice()).ok()?;
                Some(Ipv6Addr::from(raw).to_string())
            })
            .collect(),
    })
}

fn prefix_to_netmask(prefix: u32) -> String {
    let bits = match prefix {
        0 => 0,
        32.. => !0,
        _ => !0u32 << (32 - prefix),
    };
    Ipv4Addr::from(bits).to_string()
}

pub async fn hw_address(device: &DeviceInfo) -> zbus::Result<Option<String>> {
    Ok(non_empty(device.proxy.hw_address().await?))
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

pub async fn wireless_rate(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<InformationRate> {
    let wireless: WirelessProxy = uncached(connection, device.path.clone()).await?;
    let kilobits_per_second = wireless.bitrate().await?;
    Ok(InformationRate::new::<kilobit_per_second>(f64::from(
        kilobits_per_second,
    )))
}

pub struct ApInfo {
    pub ssid: Option<String>,
    pub signal: i64,
    pub frequency: Frequency,
    pub security: Option<String>,
}

pub async fn access_point(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<Option<ApInfo>> {
    let wireless: WirelessProxy = uncached(connection, device.path.clone()).await?;
    let ap_path = wireless.active_access_point().await?;
    if is_unset(&ap_path) {
        return Ok(None);
    }
    let ap: AccessPointProxy = uncached(connection, ap_path).await?;
    let (ssid, strength, frequency, flags, wpa, rsn) = try_join!(
        ap.ssid(),
        ap.strength(),
        ap.frequency(),
        ap.flags(),
        ap.wpa_flags(),
        ap.rsn_flags()
    )?;
    Ok(Some(ApInfo {
        ssid: non_empty(String::from_utf8_lossy(&ssid).into_owned()),
        signal: i64::from(strength),
        frequency: Frequency::new::<megahertz>(f64::from(frequency)),
        security: security(flags, wpa, rsn),
    }))
}

const AP_PRIVACY: u32 = 0x1;
const KEY_MGMT_8021X: u32 = 0x200;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

fn security(flags: u32, wpa: u32, rsn: u32) -> Option<String> {
    let parts = [
        (flags & AP_PRIVACY != 0 && wpa == 0 && rsn == 0).then_some("WEP"),
        (wpa != 0).then_some("WPA1"),
        (rsn & (KEY_MGMT_PSK | KEY_MGMT_8021X) != 0).then_some("WPA2"),
        (rsn & KEY_MGMT_SAE != 0).then_some("WPA3"),
        (rsn & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0).then_some("OWE"),
        (wpa & KEY_MGMT_8021X != 0 || rsn & (KEY_MGMT_8021X | KEY_MGMT_EAP_SUITE_B_192) != 0)
            .then_some("802.1X"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    (!parts.is_empty()).then_some(parts)
}

pub async fn changes(connection: &Connection) -> zbus::Result<impl Stream<Item = ()> + use<>> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(SERVICE)?
        .build();
    Ok(MessageStream::for_match_rule(rule, connection, Some(16))
        .await?
        .map(|_| ()))
}
