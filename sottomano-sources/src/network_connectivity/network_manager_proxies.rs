use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ops::RangeInclusive,
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

use crate::{network_connectivity::ReadingError, non_empty_text::non_empty};

pub const SERVICE: &str = "org.freedesktop.NetworkManager";

const STATE_ACTIVATED: u32 = 100;
const STATES_ACTIVATING: RangeInclusive<u32> = 40..=90;
const STATE_DEACTIVATING: u32 = 110;

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
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn networking_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn primary_connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn activating_connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn metered(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_wireless_enabled(&self, enabled: bool) -> zbus::Result<()>;
    #[zbus(property)]
    fn wwan_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_wwan_enabled(&self, enabled: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Device {
    fn disconnect(&self) -> zbus::Result<()>;
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
    #[zbus(property, name = "Type")]
    fn connection_type(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn vpn(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
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
    pub switching: bool,
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
        switching: STATES_ACTIVATING.contains(&state) || state == STATE_DEACTIVATING,
        path,
        proxy,
    })
}

pub struct Uplink {
    pub activated_interfaces: Vec<String>,
    pub connectivity: Option<Connectivity>,
    pub metered: bool,
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
    manager(connection).await?.networking_enabled().await
}

pub async fn manager_state(connection: &Connection) -> zbus::Result<ManagerState> {
    Ok(ManagerState::from(
        manager(connection).await?.state().await?,
    ))
}

pub async fn checked_connectivity(
    connection: &Connection,
) -> zbus::Result<Result<Connectivity, u32>> {
    Ok(Connectivity::try_from(
        manager(connection).await?.connectivity().await?,
    ))
}

pub async fn primary_uplink(connection: &Connection) -> zbus::Result<UplinkIdentity> {
    let primary_connection = manager(connection).await?.primary_connection().await?;
    uplink_identity(connection, primary_connection).await
}

pub async fn activating_uplink(connection: &Connection) -> zbus::Result<UplinkIdentity> {
    let activating_connection = manager(connection).await?.activating_connection().await?;
    uplink_identity(connection, activating_connection).await
}

async fn uplink_identity(
    connection: &Connection,
    active_connection: OwnedObjectPath,
) -> zbus::Result<UplinkIdentity> {
    if is_unset(&active_connection) {
        return Ok(UplinkIdentity::Unnamed);
    }
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

pub async fn uplink(connection: Connection) -> Result<Uplink, ReadingError> {
    let manager = manager(&connection).await?;
    let (paths, connectivity, metered) = try_join!(
        manager.get_devices(),
        manager.connectivity(),
        manager.metered()
    )?;
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
        metered: matches!(metered, METERED_YES | METERED_GUESS_YES),
    })
}

pub async fn manager_connectivity(connection: Connection) -> Result<Connectivity, ReadingError> {
    let state_code = manager(&connection).await?.connectivity().await?;
    Connectivity::try_from(state_code).map_err(|_| ReadingError::Absent)
}

pub async fn device_gateways(connection: Connection) -> Result<Vec<DeviceGateway>, ReadingError> {
    let routed_devices = devices(&connection).await?.into_iter().filter(|device| {
        device.connected && matches!(device.kind, DeviceKind::Ethernet | DeviceKind::Wifi)
    });
    let gateways_per_device = try_join_all(routed_devices.map(|device| {
        let connection = &connection;
        async move {
            let addressing = ip(connection, &device).await?;
            Ok::<_, zbus::Error>(
                [addressing.v4, addressing.v6]
                    .into_iter()
                    .flatten()
                    .filter_map(|block| block.gateway?.parse().ok())
                    .map(|gateway| DeviceGateway {
                        interface: device.interface.clone(),
                        gateway,
                    })
                    .collect::<Vec<_>>(),
            )
        }
    }))
    .await?;
    Ok(gateways_per_device.into_iter().flatten().collect())
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

pub struct WireguardProfile {
    pub settings_connection: OwnedObjectPath,
    pub name: String,
    pub interface: String,
    pub listen_port: u32,
    pub peers: Vec<WireguardPeerSettings>,
}

pub struct WireguardPeerSettings {
    pub public_key: String,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationState {
    Switching,
    Activated,
}

pub struct ActiveWireguard {
    pub settings_connection: OwnedObjectPath,
    pub active_connection: OwnedObjectPath,
    pub activation_state: ActivationState,
}

const ACTIVE_CONNECTION_ACTIVATED: u32 = 2;
const WIREGUARD_CONNECTION_TYPE: &str = "wireguard";

pub async fn wireguard_profiles(connection: &Connection) -> zbus::Result<Vec<WireguardProfile>> {
    let settings: SettingsProxy = Builder::new(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let paths = settings.list_connections().await?;
    let profiles = join_all(
        paths
            .into_iter()
            .map(|path| wireguard_profile(connection, path)),
    )
    .await;
    Ok(profiles
        .into_iter()
        .filter_map(Result::ok)
        .flatten()
        .collect())
}

fn settings_text(section: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    section
        .get(key)
        .and_then(|value| value.downcast_ref::<String>().ok())
        .and_then(non_empty)
}

async fn wireguard_profile(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<Option<WireguardProfile>> {
    let settings: SettingsConnectionProxy = uncached(connection, path.clone()).await?;
    let profile = settings.get_settings().await?;
    let (Some(connection_section), Some(wireguard_section)) = (
        profile.get("connection"),
        profile.get(WIREGUARD_CONNECTION_TYPE),
    ) else {
        return Ok(None);
    };
    let name = settings_text(connection_section, "id").unwrap_or_default();
    let interface = settings_text(connection_section, "interface-name").unwrap_or(name.clone());
    let listen_port = wireguard_section
        .get("listen-port")
        .and_then(|value| value.downcast_ref::<u32>().ok())
        .unwrap_or_default();
    let peers = wireguard_section
        .get("peers")
        .and_then(|value| {
            Vec::<HashMap<String, OwnedValue>>::try_from(value.try_clone().ok()?).ok()
        })
        .unwrap_or_default()
        .iter()
        .map(|peer_section| WireguardPeerSettings {
            public_key: settings_text(peer_section, "public-key").unwrap_or_default(),
            endpoint: settings_text(peer_section, "endpoint"),
            allowed_ips: peer_section
                .get("allowed-ips")
                .and_then(|value| Vec::<String>::try_from(value.try_clone().ok()?).ok())
                .unwrap_or_default(),
        })
        .collect();
    Ok(Some(WireguardProfile {
        settings_connection: path,
        name,
        interface,
        listen_port,
        peers,
    }))
}

pub async fn active_wireguards(connection: &Connection) -> zbus::Result<Vec<ActiveWireguard>> {
    let paths = manager(connection).await?.active_connections().await?;
    let wireguards = join_all(
        paths
            .into_iter()
            .map(|path| active_wireguard(connection, path)),
    )
    .await;
    Ok(wireguards
        .into_iter()
        .filter_map(Result::ok)
        .flatten()
        .collect())
}

async fn active_wireguard(
    connection: &Connection,
    path: OwnedObjectPath,
) -> zbus::Result<Option<ActiveWireguard>> {
    let active: ActiveConnectionProxy = uncached(connection, path.clone()).await?;
    if active.connection_type().await? != WIREGUARD_CONNECTION_TYPE {
        return Ok(None);
    }
    let (settings_connection, state_code) = try_join!(active.connection(), active.state())?;
    Ok(Some(ActiveWireguard {
        settings_connection,
        active_connection: path,
        activation_state: if state_code == ACTIVE_CONNECTION_ACTIVATED {
            ActivationState::Activated
        } else {
            ActivationState::Switching
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

pub async fn wireless_enabled(connection: &Connection) -> zbus::Result<bool> {
    manager(connection).await?.wireless_enabled().await
}

pub async fn toggle_wireless_radio(connection: &Connection) -> zbus::Result<()> {
    let manager = manager(connection).await?;
    let enabled = manager.wireless_enabled().await?;
    manager.set_wireless_enabled(!enabled).await
}

pub async fn toggle_wwan_radio(connection: &Connection) -> zbus::Result<()> {
    let manager = manager(connection).await?;
    let enabled = manager.wwan_enabled().await?;
    manager.set_wwan_enabled(!enabled).await
}

pub async fn disconnect_device(device: &DeviceInfo) -> zbus::Result<()> {
    device.proxy.disconnect().await
}

pub async fn connect_device(connection: &Connection, device: &DeviceInfo) -> zbus::Result<()> {
    let unset = ObjectPath::from_static_str_unchecked("/");
    manager(connection)
        .await?
        .activate_connection(&unset.clone().into(), &device.path, &unset)
        .await
        .map(drop)
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
pub const CONNECTIVITY_PROPERTIES: &[&str] = &[MANAGER_PROPERTIES];
pub const INTERNET_ACCESS_PROPERTIES: &[&str] = &[MANAGER_PROPERTIES, ACTIVE_CONNECTION_PROPERTIES];
pub const GATEWAY_PROPERTIES: &[&str] = &[
    MANAGER_PROPERTIES,
    DEVICE_PROPERTIES,
    IP4_CONFIG_PROPERTIES,
    IP6_CONFIG_PROPERTIES,
];

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
