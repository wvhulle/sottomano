use std::collections::HashMap;

use futures::{StreamExt, stream, try_join};
use zbus::{
    Connection,
    proxy::{Builder, CacheProperties},
    zvariant::{OwnedObjectPath, OwnedValue},
};

use super::{
    ActiveConnectionProxy, BUS_OBJECT_READS_IN_FLIGHT, IpInfo, SettingsConnectionProxy,
    SettingsProxy, VpnConnectionProxy, network_manager_proxy,
    nm_ip_configs::{ip4_config, ip6_config},
    readable_objects, settings_text, uncached,
};
use crate::{
    network_connectivity::wireguard_tunnels::PeerEndpoint,
    system_protocol_clients::dbus_desktop_buses::prop,
};

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
    let profiles = stream::iter(paths)
        .map(|path| vpn_profile(connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .collect::<Vec<_>>()
        .await;
    Ok(readable_objects(profiles, "vpn profile")
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
        .and_then(|section| prop::<String>(section, "id"))
        .unwrap_or_default();
    let service_type = prop::<String>(vpn_section, "service-type").unwrap_or_default();
    let vpn_data = prop::<HashMap<String, String>>(vpn_section, "data").unwrap_or_default();
    Ok(Some(VpnProfile {
        settings_connection: path,
        name,
        service_type,
        vpn_data,
    }))
}

pub async fn active_vpns(connection: &Connection) -> zbus::Result<Vec<ActiveVpn>> {
    let paths = network_manager_proxy(connection)
        .await?
        .active_connections()
        .await?;
    let vpns = stream::iter(paths)
        .map(|path| active_vpn(connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .collect::<Vec<_>>()
        .await;
    Ok(readable_objects(vpns, "active vpn").flatten().collect())
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
    pub endpoint: PeerEndpoint,
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
    let profiles = stream::iter(paths)
        .map(|path| wireguard_profile(connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .collect::<Vec<_>>()
        .await;
    Ok(readable_objects(profiles, "wireguard profile")
        .flatten()
        .collect())
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
    let listen_port = prop::<u32>(wireguard_section, "listen-port").unwrap_or_default();
    let peers = prop::<Vec<HashMap<String, OwnedValue>>>(wireguard_section, "peers")
        .unwrap_or_default()
        .iter()
        .map(|peer_section| WireguardPeerSettings {
            public_key: settings_text(peer_section, "public-key").unwrap_or_default(),
            endpoint: PeerEndpoint::from(settings_text(peer_section, "endpoint")),
            allowed_ips: prop::<Vec<String>>(peer_section, "allowed-ips").unwrap_or_default(),
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
    let paths = network_manager_proxy(connection)
        .await?
        .active_connections()
        .await?;
    let wireguards = stream::iter(paths)
        .map(|path| active_wireguard(connection, path))
        .buffered(BUS_OBJECT_READS_IN_FLIGHT)
        .collect::<Vec<_>>()
        .await;
    Ok(readable_objects(wireguards, "active wireguard")
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
