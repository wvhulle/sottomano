use std::{
    collections::HashMap,
    net::{Ipv4Addr, Ipv6Addr},
};

use futures::{
    Stream, StreamExt,
    future::{join_all, try_join_all},
    stream, try_join,
};
use uom::si::{
    f64::{Frequency, InformationRate},
    frequency::megahertz,
    information_rate::{kilobit_per_second, megabit_per_second},
};
use zbus::{
    Connection, MatchRule, MessageStream, match_rule,
    message::Type,
    proxy,
    proxy::{Builder, CacheProperties, Defaults},
    zvariant::{ObjectPath, OwnedObjectPath, OwnedValue},
};

use crate::{
    network_connectivity::ReadingError, non_empty_text::non_empty,
    stream_operators::connected_until_failure,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    Full,
    Limited,
    Offline,
}

impl TryFrom<u32> for Connectivity {
    type Error = u32;

    fn try_from(state_code: u32) -> Result<Self, Self::Error> {
        match state_code {
            4 => Ok(Self::Full),
            2 | 3 => Ok(Self::Limited),
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
            Self::Limited => "limited",
            Self::Offline => "none",
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
    fn deactivate_connection(&self, active_connection: &OwnedObjectPath) -> zbus::Result<()>;
    fn activate_connection(
        &self,
        settings_connection: &OwnedObjectPath,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
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
    #[zbus(property)]
    fn vpn(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn ip4_config(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn ip6_config(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.VPN.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait VpnConnection {
    #[zbus(property)]
    fn vpn_state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn banner(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait SettingsConnection {
    fn get_settings(&self) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
trait Settings {
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
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
    pub connectivity: Option<Connectivity>,
}

pub fn connected(system: Connection) -> impl Stream<Item = bool> + Send {
    connected_until_failure(connectivity_changes(system)).map(|connected| {
        connected.unwrap_or_else(|e| {
            tracing::warn!("network connectivity watch: {e}");
            false
        })
    })
}

async fn connectivity_changes(
    system: Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<bool>> + Send> {
    let manager = ManagerProxy::new(&system).await?;
    let full = |code: u32| Connectivity::try_from(code) == Ok(Connectivity::Full);
    let now = full(manager.connectivity().await?);
    let changes = manager
        .receive_connectivity_changed()
        .await
        .then(move |change| async move { change.get().await.map(full) });
    Ok(stream::iter([Ok(now)]).chain(changes))
}

pub async fn uplink(connection: Connection) -> Result<Uplink, ReadingError> {
    let manager = manager(&connection).await?;
    let (paths, connectivity) = try_join!(manager.get_devices(), manager.connectivity())?;
    let interfaces = try_join_all(
        paths
            .into_iter()
            .map(|path| device_interface(&connection, path)),
    )
    .await?;
    Ok(Uplink {
        activated_interfaces: interfaces
            .into_iter()
            .filter(|interface| interface.activated && !interface.name.is_empty())
            .map(|interface| interface.name)
            .collect(),
        connectivity: Connectivity::try_from(connectivity).ok(),
    })
}

struct DeviceInterface {
    activated: bool,
    name: String,
}

async fn device_interface(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<DeviceInterface> {
    let device: DeviceProxy = uncached(connection, path).await?;
    let (state, name) = try_join!(device.state(), device.ip_interface())?;
    Ok(DeviceInterface {
        activated: state == STATE_ACTIVATED,
        name,
    })
}

pub async fn active_connection(device: &DeviceInfo) -> zbus::Result<OwnedObjectPath> {
    device.proxy.active_connection().await
}

pub async fn connection_id(
    connection: &Connection,
    active_connection: OwnedObjectPath,
) -> zbus::Result<String> {
    let active: ActiveConnectionProxy = uncached(connection, active_connection).await?;
    active.id().await
}

pub struct VpnProfile {
    pub settings_connection: OwnedObjectPath,
    pub name: String,
    pub service_type: String,
    pub vpn_data: HashMap<String, String>,
}

pub struct ActiveVpn {
    pub settings_connection: OwnedObjectPath,
    pub active_connection: OwnedObjectPath,
    pub vpn_state_code: u32,
    pub banner: String,
    pub ip: IpInfo,
}

pub async fn vpn_profiles(connection: &Connection) -> zbus::Result<Vec<VpnProfile>> {
    let settings: SettingsProxy = Builder::new(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let paths = settings.list_connections().await?;
    let profiles = join_all(paths.into_iter().map(|path| vpn_profile(connection, path))).await;
    Ok(profiles
        .into_iter()
        .filter_map(Result::ok)
        .flatten()
        .collect())
}

async fn vpn_profile(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<Option<VpnProfile>> {
    let settings: SettingsConnectionProxy = uncached(connection, path.clone()).await?;
    let profile = settings.get_settings().await?;
    let Some(vpn_section) = profile.get("vpn") else {
        return Ok(None);
    };
    let name = profile
        .get("connection")
        .and_then(|section| section.get("id"))
        .and_then(|value| value.downcast_ref::<String>().ok())
        .unwrap_or_default();
    let service_type = vpn_section
        .get("service-type")
        .and_then(|value| value.downcast_ref::<String>().ok())
        .unwrap_or_default();
    let vpn_data = vpn_section
        .get("data")
        .and_then(|value| HashMap::<String, String>::try_from(value.try_clone().ok()?).ok())
        .unwrap_or_default();
    Ok(Some(VpnProfile {
        settings_connection: path,
        name,
        service_type,
        vpn_data,
    }))
}

pub async fn active_vpns(connection: &Connection) -> zbus::Result<Vec<ActiveVpn>> {
    let paths = manager(connection).await?.active_connections().await?;
    let vpns = join_all(paths.into_iter().map(|path| active_vpn(connection, path))).await;
    Ok(vpns.into_iter().filter_map(Result::ok).flatten().collect())
}

async fn active_vpn(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<Option<ActiveVpn>> {
    let active: ActiveConnectionProxy = uncached(connection, path.clone()).await?;
    if !active.vpn().await? {
        return Ok(None);
    }
    let vpn: VpnConnectionProxy = uncached(connection, path.clone()).await?;
    let (vpn_state_code, banner, settings_connection, v4_path, v6_path) = try_join!(
        vpn.vpn_state(),
        vpn.banner(),
        active.connection(),
        active.ip4_config(),
        active.ip6_config()
    )?;
    let (v4, v6) = try_join!(
        ip4_config(connection, v4_path),
        ip6_config(connection, v6_path)
    )?;
    Ok(Some(ActiveVpn {
        settings_connection,
        active_connection: path,
        vpn_state_code,
        banner,
        ip: IpInfo {
            v4: v4.block,
            v6: v6.block,
            dns: v4.dns.into_iter().chain(v6.dns).collect(),
        },
    }))
}

pub async fn activate(
    connection: &Connection,
    settings_connection: &OwnedObjectPath,
) -> zbus::Result<()> {
    let unset = ObjectPath::from_static_str_unchecked("/");
    manager(connection)
        .await?
        .activate_connection(settings_connection, &unset, &unset)
        .await
        .map(drop)
}

pub async fn deactivate(
    connection: &Connection,
    active_connection: &OwnedObjectPath,
) -> zbus::Result<()> {
    manager(connection)
        .await?
        .deactivate_connection(active_connection)
        .await
}

#[must_use]
pub fn is_unset(path: &OwnedObjectPath) -> bool {
    path.as_str() == "/"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressBlock {
    pub address: String,
    pub mask: String,
    pub gateway: Option<String>,
}

#[derive(Debug)]
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
    pub security: String,
}

pub async fn active_access_point(
    connection: &Connection,
    device: &DeviceInfo,
) -> zbus::Result<OwnedObjectPath> {
    let wireless: WirelessProxy = uncached(connection, device.path.clone()).await?;
    wireless.active_access_point().await
}

pub async fn access_point(
    connection: &Connection,
    ap_path: OwnedObjectPath,
) -> zbus::Result<ApInfo> {
    let ap: AccessPointProxy = uncached(connection, ap_path).await?;
    let (ssid, strength, frequency, flags, wpa, rsn) = try_join!(
        ap.ssid(),
        ap.strength(),
        ap.frequency(),
        ap.flags(),
        ap.wpa_flags(),
        ap.rsn_flags()
    )?;
    Ok(ApInfo {
        ssid: non_empty(String::from_utf8_lossy(&ssid).into_owned()),
        signal: i64::from(strength),
        frequency: Frequency::new::<megahertz>(f64::from(frequency)),
        security: security(flags, wpa, rsn),
    })
}

const AP_PRIVACY: u32 = 0x1;
const KEY_MGMT_8021X: u32 = 0x200;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

fn security(flags: u32, wpa: u32, rsn: u32) -> String {
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
    if parts.is_empty() {
        "Open".to_owned()
    } else {
        parts
    }
}

pub const MANAGER_PROPERTIES: &str = "org.freedesktop.NetworkManager";
pub const DEVICE_PROPERTIES: &str = "org.freedesktop.NetworkManager.Device";
pub const WIRED_PROPERTIES: &str = "org.freedesktop.NetworkManager.Device.Wired";
pub const WIRELESS_PROPERTIES: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACCESS_POINT_PROPERTIES: &str = "org.freedesktop.NetworkManager.AccessPoint";
pub const IP4_CONFIG_PROPERTIES: &str = "org.freedesktop.NetworkManager.IP4Config";
pub const IP6_CONFIG_PROPERTIES: &str = "org.freedesktop.NetworkManager.IP6Config";
pub const ACTIVE_CONNECTION_PROPERTIES: &str = "org.freedesktop.NetworkManager.Connection.Active";
pub const VPN_CONNECTION_PROPERTIES: &str = "org.freedesktop.NetworkManager.VPN.Connection";
pub const SETTINGS_PROPERTIES: &str = "org.freedesktop.NetworkManager.Settings";

pub const UPLINK_PROPERTIES: &[&str] = &[MANAGER_PROPERTIES, DEVICE_PROPERTIES];

pub async fn changes(
    connection: &Connection,
    watched_properties: &'static [&'static str],
) -> zbus::Result<impl Stream<Item = ()> + use<>> {
    let streams = try_join_all(watched_properties.iter().map(|interface| async move {
        let rule = properties_changed_rule(interface)?.build();
        MessageStream::for_match_rule(rule, connection, Some(16)).await
    }))
    .await?;
    Ok(stream::select_all(streams).map(|_| ()))
}

pub async fn access_point_changes(
    connection: &Connection,
    access_point: OwnedObjectPath,
) -> zbus::Result<impl Stream<Item = ()> + use<>> {
    let rule = properties_changed_rule(ACCESS_POINT_PROPERTIES)?
        .path(access_point)?
        .build();
    Ok(MessageStream::for_match_rule(rule, connection, Some(16))
        .await?
        .map(|_| ()))
}

fn properties_changed_rule(interface: &str) -> zbus::Result<match_rule::Builder<'_>> {
    MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(SERVICE)?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .add_arg(interface)
}
