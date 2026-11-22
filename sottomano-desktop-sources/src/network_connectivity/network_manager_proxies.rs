mod nm_device_listing;
mod nm_ip_configs;
mod nm_uplink_state;
mod nm_vpn_wireguard_profiles;
mod nm_wireless_access_points;

use std::collections::HashMap;

use futures::Stream;
pub use nm_device_listing::{
    DeviceInfo, DeviceKind, LinkPhase, active_connection, devices, devices_of_kind, hw_address,
    modem_device_at, mtu, wired_speed,
};
pub use nm_ip_configs::{AddressAssignment, AddressBlock, FamilyConfig, IpInfo, RouterGateway, ip};
pub use nm_uplink_state::{
    ActiveConnectionUplink, ConnectedScope, Connectivity, DeviceGateway, ManagerState, Metering,
    RadioPower, Uplink, UplinkIdentity, activating_uplink, checked_connectivity, connection_id,
    device_gateways, manager_connectivity, manager_state, networking_enabled, primary_uplink,
    uplink, wireless_radio_power,
};
pub use nm_vpn_wireguard_profiles::{
    ActivationState, ActiveVpn, ActiveWireguard, VpnProfile, WireguardPeerSettings,
    WireguardProfile, active_vpns, active_wireguards, vpn_profiles, wireguard_profiles,
};
pub use nm_wireless_access_points::{
    ApInfo, NetworkName, WirelessProfile, access_point, active_access_point,
    available_wireless_profiles, request_wireless_scan, visible_access_points, wireless_rate,
};
use zbus::{
    Connection, proxy,
    proxy::{Builder, CacheProperties, Defaults},
    zvariant::{ObjectPath, OwnedObjectPath, OwnedValue},
};

use crate::{
    non_empty_text::non_empty,
    system_protocol_clients::{
        dbus_change_signals::{change_triggers, properties_changed_rule},
        dbus_desktop_buses::prop,
    },
};

pub const BUS_OBJECT_READS_IN_FLIGHT: usize = 8;

pub const SERVICE: &str = "org.freedesktop.NetworkManager";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NmConnectionSwitch {
    Connect(OwnedObjectPath),
    Disconnect(OwnedObjectPath),
}

#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager",
    gen_blocking = false
)]
pub trait Manager {
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
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
)]
pub trait Device {
    fn disconnect(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn udi(&self) -> zbus::Result<String>;
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
    #[zbus(property)]
    fn available_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wired",
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
)]
trait Wired {
    #[zbus(property)]
    fn speed(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
)]
trait Wireless {
    fn get_all_access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    fn request_scan(&self, options: HashMap<&str, OwnedValue>) -> zbus::Result<()>;
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn bitrate(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
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
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
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
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
)]
trait VpnConnection {
    #[zbus(property)]
    fn vpn_state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn banner(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
)]
trait SettingsConnection {
    fn get_settings(&self) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings",
    gen_blocking = false
)]
trait Settings {
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.IP4Config",
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
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
    default_service = "org.freedesktop.NetworkManager",
    gen_blocking = false
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

pub async fn network_manager_proxy(connection: &Connection) -> zbus::Result<ManagerProxy<'static>> {
    Builder::new(connection)
        .cache_properties(CacheProperties::No)
        .build()
        .await
}

#[must_use]
pub fn assigned_object_path(path: OwnedObjectPath) -> Option<OwnedObjectPath> {
    (path.as_str() != "/").then_some(path)
}

fn readable_objects<ObjectSample>(
    object_reads: Vec<zbus::Result<ObjectSample>>,
    object_kind: &'static str,
) -> impl Iterator<Item = ObjectSample> {
    object_reads.into_iter().filter_map(move |object_read| {
        object_read
            .inspect_err(|read_failure| {
                tracing::debug!(
                    "network manager: skipping unreadable {object_kind}: {read_failure}"
                );
            })
            .ok()
    })
}

fn settings_text(section: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    prop::<String>(section, key).and_then(non_empty)
}

pub const MANAGER_PROPERTIES: &str = "org.freedesktop.NetworkManager";
pub const DEVICE_PROPERTIES: &str = "org.freedesktop.NetworkManager.Device";
pub const WIRED_PROPERTIES: &str = "org.freedesktop.NetworkManager.Device.Wired";
pub const WIRELESS_PROPERTIES: &str = "org.freedesktop.NetworkManager.Device.Wireless";
pub const ACCESS_POINT_PROPERTIES: &str = "org.freedesktop.NetworkManager.AccessPoint";
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
) -> zbus::Result<impl Stream<Item = zbus::Result<()>> + use<>> {
    let rules = watched_properties
        .iter()
        .map(|interface| properties_changed_rule(SERVICE, interface))
        .collect::<zbus::Result<Vec<_>>>()?;
    change_triggers(connection, rules).await
}

fn shown_ssid(ssid_bytes: Vec<u8>) -> String {
    String::from_utf8(ssid_bytes)
        .unwrap_or_else(|non_utf8_ssid| non_utf8_ssid.as_bytes().escape_ascii().to_string())
}
