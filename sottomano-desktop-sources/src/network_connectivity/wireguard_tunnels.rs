mod tunnel_interface_addresses;
mod wireguard_kernel_dumps;
mod wireguard_tunnel_assembly;
#[cfg(test)]
mod wireguard_tunnels_tests;

use std::time::Duration;

use bytesize::ByteSize;
use zbus::zvariant::OwnedObjectPath;

use crate::network_connectivity::network_manager_proxies::{
    LinkPhase, NmConnectionSwitch, WireguardPeerSettings,
};

const CONNECTED_WINDOW: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEndpoint {
    Roaming,
    At(String),
}

impl From<Option<String>> for PeerEndpoint {
    fn from(reported: Option<String>) -> Self {
        reported.map_or(Self::Roaming, Self::At)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerStatistics {
    pub handshake_ago: Duration,
    pub received: ByteSize,
    pub sent: ByteSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerTraffic {
    Unobserved,
    NeverHandshaken,
    Exchanged(PeerStatistics),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerLink {
    Connected,
    Stale,
}

impl PeerTraffic {
    #[must_use]
    pub fn peer_link(self) -> PeerLink {
        match self {
            Self::Exchanged(statistics) if statistics.handshake_ago <= CONNECTED_WINDOW => {
                PeerLink::Connected
            }
            Self::Exchanged(_) | Self::Unobserved | Self::NeverHandshaken => PeerLink::Stale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteScope {
    AllTraffic,
    SingleHosts,
    Subnets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub public_key: String,
    pub endpoint: PeerEndpoint,
    pub allowed_ips: Vec<String>,
    pub traffic: PeerTraffic,
}

impl Peer {
    #[must_use]
    pub fn route_scope(&self) -> RouteScope {
        if self
            .allowed_ips
            .iter()
            .any(|allowed_ip| matches!(allowed_ip.as_str(), "0.0.0.0/0" | "::/0"))
        {
            RouteScope::AllTraffic
        } else if !self.allowed_ips.is_empty()
            && self
                .allowed_ips
                .iter()
                .all(|allowed_ip| allowed_ip.ends_with("/32") || allowed_ip.ends_with("/128"))
        {
            RouteScope::SingleHosts
        } else {
            RouteScope::Subnets
        }
    }

    fn configured(peer_settings: WireguardPeerSettings) -> Self {
        Self {
            public_key: peer_settings.public_key,
            endpoint: peer_settings.endpoint,
            allowed_ips: peer_settings.allowed_ips,
            traffic: PeerTraffic::Unobserved,
        }
    }

    fn observed_by(self, kernel_peers: &[Peer]) -> Self {
        let Some(kernel_peer) = kernel_peers
            .iter()
            .find(|kernel_peer| kernel_peer.public_key == self.public_key)
        else {
            return self;
        };
        Self {
            endpoint: match &kernel_peer.endpoint {
                PeerEndpoint::At(_) => kernel_peer.endpoint.clone(),
                PeerEndpoint::Roaming => self.endpoint,
            },
            traffic: kernel_peer.traffic,
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelRole {
    Server { listen_port: u32 },
    Client,
    Undetermined,
}

impl TunnelRole {
    fn configured(listen_port: u32) -> Self {
        if listen_port > 0 {
            Self::Server { listen_port }
        } else {
            Self::Client
        }
    }

    fn inferred_from_peers(listen_port: u32, peers: &[Peer]) -> Self {
        if peers.len() > 1
            || peers
                .first()
                .is_some_and(|peer| peer.route_scope() == RouteScope::SingleHosts)
        {
            Self::Server { listen_port }
        } else {
            Self::Client
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelActivity {
    Inactive,
    Switching { active_connection: OwnedObjectPath },
    Active { active_connection: OwnedObjectPath },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelOrigin {
    NetworkManager {
        settings_connection: OwnedObjectPath,
        activity: TunnelActivity,
    },
    KernelOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatisticsAccess {
    TunnelDown,
    NeedsNetAdmin,
    Unreadable(String),
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tunnel {
    pub name: String,
    pub interface: String,
    pub role: TunnelRole,
    pub origin: TunnelOrigin,
    pub statistics_access: StatisticsAccess,
    pub peers: Vec<Peer>,
}

impl Tunnel {
    #[must_use]
    pub fn connected_peers(&self) -> usize {
        self.peers
            .iter()
            .filter(|peer| peer.traffic.peer_link() == PeerLink::Connected)
            .count()
    }

    #[must_use]
    pub fn tunnel_phase(&self) -> LinkPhase {
        match &self.origin {
            TunnelOrigin::NetworkManager {
                activity: TunnelActivity::Active { .. },
                ..
            }
            | TunnelOrigin::KernelOnly => LinkPhase::Up,
            TunnelOrigin::NetworkManager {
                activity: TunnelActivity::Switching { .. },
                ..
            } => LinkPhase::Switching,
            TunnelOrigin::NetworkManager {
                activity: TunnelActivity::Inactive,
                ..
            } => LinkPhase::Down,
        }
    }

    #[must_use]
    pub fn traffic_routing(&self) -> TunnelRouting {
        let default_route = self.role == TunnelRole::Client
            && self.tunnel_phase() == LinkPhase::Up
            && self
                .peers
                .iter()
                .any(|peer| peer.route_scope() == RouteScope::AllTraffic);
        if default_route {
            TunnelRouting::DefaultRoute
        } else {
            TunnelRouting::Partial
        }
    }

    #[must_use]
    pub fn switch(&self) -> Option<NmConnectionSwitch> {
        let TunnelOrigin::NetworkManager {
            settings_connection,
            activity,
        } = &self.origin
        else {
            return None;
        };
        Some(match activity {
            TunnelActivity::Inactive => NmConnectionSwitch::Connect(settings_connection.clone()),
            TunnelActivity::Switching { active_connection }
            | TunnelActivity::Active { active_connection } => {
                NmConnectionSwitch::Disconnect(active_connection.clone())
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelAddress {
    Assigned(String),
    Unassigned,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wireguard {
    pub tunnels: Vec<Tunnel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelRouting {
    DefaultRoute,
    Partial,
}
