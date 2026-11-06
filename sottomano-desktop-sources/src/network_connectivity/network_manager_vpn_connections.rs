use futures::{Stream, try_join};
use zbus::{Connection, zvariant::OwnedObjectPath};

use crate::{
    SnapshotDelivery, SnapshotSource,
    network_connectivity::{
        ReadingError, device_or_absent,
        network_manager_proxies::{self, ActiveVpn, AddressBlock, VpnProfile},
        nm_readings,
    },
    non_empty_text::non_empty,
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
};

const PLUGIN_SERVICE_PREFIX: &str = "org.freedesktop.NetworkManager.";

const WATCHED_VPN_PROPERTIES: &[&str] = &[
    network_manager_proxies::MANAGER_PROPERTIES,
    network_manager_proxies::SETTINGS_PROPERTIES,
    network_manager_proxies::ACTIVE_CONNECTION_PROPERTIES,
    network_manager_proxies::VPN_CONNECTION_PROPERTIES,
    network_manager_proxies::IP4_CONFIG_PROPERTIES,
    network_manager_proxies::IP6_CONFIG_PROPERTIES,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VpnState {
    Preparing,
    NeedsAuthentication,
    Connecting,
    AwaitingAddress,
    Activated,
    Failed,
    Disconnected,
}

impl From<u32> for VpnState {
    fn from(state_code: u32) -> Self {
        match state_code {
            1 => Self::Preparing,
            2 => Self::NeedsAuthentication,
            3 => Self::Connecting,
            4 => Self::AwaitingAddress,
            5 => Self::Activated,
            6 => Self::Failed,
            _ => Self::Disconnected,
        }
    }
}

impl VpnState {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::NeedsAuthentication => "needs authentication",
            Self::Connecting => "connecting",
            Self::AwaitingAddress => "getting an address",
            Self::Activated => "connected",
            Self::Failed => "failed",
            Self::Disconnected => "disconnected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveTunnel {
    pub vpn_state: VpnState,
    pub ipv4: Option<AddressBlock>,
    pub ipv6: Option<AddressBlock>,
    pub dns: Vec<String>,
    pub banner: Option<String>,
    pub active_connection: OwnedObjectPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VpnActivity {
    Inactive,
    Active(Box<ActiveTunnel>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VpnSwitch {
    Connect(OwnedObjectPath),
    Disconnect(OwnedObjectPath),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnConnection {
    pub name: String,
    pub plugin: String,
    pub protocol: Option<String>,
    pub remote_gateway: Option<String>,
    pub settings_connection: OwnedObjectPath,
    pub activity: VpnActivity,
}

impl VpnConnection {
    #[must_use]
    pub fn protocol_label(&self) -> String {
        match self.protocol.as_deref() {
            Some("gp") => "GlobalProtect".to_owned(),
            Some("anyconnect") => "Cisco AnyConnect".to_owned(),
            Some("nc") => "Juniper Network Connect".to_owned(),
            Some("pulse") => "Pulse Secure".to_owned(),
            Some("f5") => "F5 BIG-IP".to_owned(),
            Some("fortinet") => "FortiGate".to_owned(),
            Some("array") => "Array Networks".to_owned(),
            Some(other) => other.to_owned(),
            None => self.plugin.clone(),
        }
    }

    #[must_use]
    pub fn switch(&self) -> VpnSwitch {
        match &self.activity {
            VpnActivity::Inactive => VpnSwitch::Connect(self.settings_connection.clone()),
            VpnActivity::Active(tunnel) => VpnSwitch::Disconnect(tunnel.active_connection.clone()),
        }
    }

    #[must_use]
    pub fn activated(&self) -> bool {
        matches!(&self.activity, VpnActivity::Active(tunnel) if tunnel.vpn_state == VpnState::Activated)
    }

    fn from_profile(profile: VpnProfile, active_vpns: &mut Vec<ActiveVpn>) -> Self {
        let plugin = profile
            .service_type
            .strip_prefix(PLUGIN_SERVICE_PREFIX)
            .unwrap_or(&profile.service_type)
            .to_owned();
        let data_field = |key: &str| profile.vpn_data.get(key).cloned().and_then(non_empty);
        let activity = active_vpns
            .iter()
            .position(|active_vpn| active_vpn.settings_connection == profile.settings_connection)
            .map(|position| active_vpns.swap_remove(position))
            .map_or(VpnActivity::Inactive, |active_vpn| {
                VpnActivity::Active(Box::new(ActiveTunnel {
                    vpn_state: VpnState::from(active_vpn.vpn_state_code),
                    ipv4: active_vpn.ip.v4,
                    ipv6: active_vpn.ip.v6,
                    dns: active_vpn.ip.dns,
                    banner: non_empty(active_vpn.banner.trim().to_owned()),
                    active_connection: active_vpn.active_connection,
                }))
            });
        Self {
            protocol: data_field("protocol"),
            remote_gateway: data_field("gateway").or_else(|| data_field("remote")),
            name: profile.name,
            plugin,
            settings_connection: profile.settings_connection,
            activity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkManagerVpns {
    pub connections: Vec<VpnConnection>,
}

async fn read(connection: Connection) -> Result<NetworkManagerVpns, ReadingError> {
    let (profiles, mut active_vpns) = try_join!(
        network_manager_proxies::vpn_profiles(&connection),
        network_manager_proxies::active_vpns(&connection)
    )?;
    if profiles.is_empty() {
        return Err(ReadingError::Absent);
    }
    Ok(NetworkManagerVpns {
        connections: profiles
            .into_iter()
            .map(|profile| VpnConnection::from_profile(profile, &mut active_vpns))
            .collect(),
    })
}

impl SnapshotSource for NetworkManagerVpns {
    type Error = zbus::Error;
    type StreamInputs = DesktopBuses;

    const NAME: &'static str = "NetworkManager VPN connections";

    fn delivery_stream(buses: DesktopBuses) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        let system = buses.system;
        device_or_absent(
            system.clone(),
            network_manager_proxies::SERVICE,
            move || nm_readings(system.clone(), WATCHED_VPN_PROPERTIES, read),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::network_connectivity::network_manager_proxies::IpInfo;

    fn object_path(path: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(path).unwrap()
    }

    fn globalprotect_profile() -> VpnProfile {
        VpnProfile {
            settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/1"),
            name: "Company VPN".to_owned(),
            service_type: "org.freedesktop.NetworkManager.openconnect".to_owned(),
            vpn_data: HashMap::from([
                ("protocol".to_owned(), "gp".to_owned()),
                ("gateway".to_owned(), "vpn.example.com".to_owned()),
            ]),
        }
    }

    fn active_globalprotect(vpn_state_code: u32) -> ActiveVpn {
        ActiveVpn {
            settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/1"),
            active_connection: object_path("/org/freedesktop/NetworkManager/ActiveConnection/7"),
            vpn_state_code,
            banner: "  Welcome  \n".to_owned(),
            ip: IpInfo {
                v4: None,
                v6: None,
                dns: vec!["10.0.0.53".to_owned()],
            },
        }
    }

    #[test]
    fn active_profile_maps_plugin_protocol_gateway_and_tunnel() {
        let mut active_vpns = vec![active_globalprotect(5)];
        let vpn_connection = VpnConnection::from_profile(globalprotect_profile(), &mut active_vpns);
        assert_eq!(vpn_connection.plugin, "openconnect");
        assert_eq!(vpn_connection.protocol_label(), "GlobalProtect");
        assert_eq!(
            vpn_connection.remote_gateway.as_deref(),
            Some("vpn.example.com")
        );
        assert!(vpn_connection.activated());
        assert_eq!(
            vpn_connection.switch(),
            VpnSwitch::Disconnect(object_path(
                "/org/freedesktop/NetworkManager/ActiveConnection/7"
            ))
        );
        let VpnActivity::Active(tunnel) = vpn_connection.activity else {
            panic!("profile with an active tunnel reads as inactive");
        };
        assert_eq!(tunnel.banner.as_deref(), Some("Welcome"));
        assert!(active_vpns.is_empty());
    }

    #[test]
    fn inactive_profile_switches_to_connect() {
        let vpn_connection = VpnConnection::from_profile(globalprotect_profile(), &mut Vec::new());
        assert_eq!(vpn_connection.activity, VpnActivity::Inactive);
        assert!(!vpn_connection.activated());
        assert_eq!(
            vpn_connection.switch(),
            VpnSwitch::Connect(object_path("/org/freedesktop/NetworkManager/Settings/1"))
        );
    }

    #[test]
    fn state_codes_follow_network_manager_vpn_connection_state() {
        let vpn_states: Vec<VpnState> = (0..=7).map(VpnState::from).collect();
        assert_eq!(
            vpn_states,
            [
                VpnState::Disconnected,
                VpnState::Preparing,
                VpnState::NeedsAuthentication,
                VpnState::Connecting,
                VpnState::AwaitingAddress,
                VpnState::Activated,
                VpnState::Failed,
                VpnState::Disconnected,
            ]
        );
    }

    #[test]
    fn plugin_without_protocol_falls_back_to_plugin_name() {
        let mut openvpn = globalprotect_profile();
        openvpn.service_type = "org.freedesktop.NetworkManager.openvpn".to_owned();
        openvpn.vpn_data =
            HashMap::from([("remote".to_owned(), "vpn.example.org:1194".to_owned())]);
        let vpn_connection = VpnConnection::from_profile(openvpn, &mut Vec::new());
        assert_eq!(vpn_connection.protocol_label(), "openvpn");
        assert_eq!(
            vpn_connection.remote_gateway.as_deref(),
            Some("vpn.example.org:1194")
        );
    }
}
