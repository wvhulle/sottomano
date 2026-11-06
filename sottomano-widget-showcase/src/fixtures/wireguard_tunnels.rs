use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    network_connectivity::wireguard_tunnels::{
        self, PeerEndpoint, PeerStatistics, PeerTraffic, StatisticsAccess, Tunnel, TunnelActivity,
        TunnelAddress, TunnelOrigin, TunnelRole, Wireguard,
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::wireguard_tunnels_group::{
        WireguardTunnelTarget, WireguardTunnelsGroup, WireguardTunnelsMessage,
    },
};
use zbus::zvariant::OwnedObjectPath;

use crate::showcased_group::ShowcasedGroup;

pub const HOME_SERVER_TUNNEL_NAME: &str = "wg-home";
const LAB_TUNNEL_NAME: &str = "wg-lab";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut tunnels_group = WireguardTunnelsGroup::default();
    let _ = tunnels_group.update(
        WireguardTunnelsMessage::Wireguard(SnapshotDelivery::fixture_delivered(
            Ok(Wireguard {
                tunnels: vec![home_wireguard_server(), lab_wireguard_tunnel()],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::WireguardTunnels(Box::new(tunnels_group))
}

fn object_path(path: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(path).expect("static object path")
}

fn exchanged_traffic(handshake_seconds: u64, received: ByteSize, sent: ByteSize) -> PeerTraffic {
    PeerTraffic::Exchanged(PeerStatistics {
        handshake_ago: Duration::from_secs(handshake_seconds),
        received,
        sent,
    })
}

fn lab_wireguard_tunnel() -> Tunnel {
    Tunnel {
        name: LAB_TUNNEL_NAME.to_owned(),
        interface: LAB_TUNNEL_NAME.to_owned(),
        role: TunnelRole::Client,
        origin: TunnelOrigin::NetworkManager {
            settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/14"),
            activity: TunnelActivity::Active {
                active_connection: object_path(
                    "/org/freedesktop/NetworkManager/ActiveConnection/9",
                ),
            },
        },
        statistics_access: StatisticsAccess::Read,
        peers: vec![wireguard_tunnels::Peer {
            public_key: "lab-gateway-key".to_owned(),
            endpoint: PeerEndpoint::At("203.0.113.24:51820".to_owned()),
            allowed_ips: vec!["10.42.0.0/24".to_owned()],
            traffic: exchanged_traffic(37, ByteSize::mib(84), ByteSize::mib(12)),
        }],
    }
}

fn home_wireguard_server() -> Tunnel {
    Tunnel {
        name: HOME_SERVER_TUNNEL_NAME.to_owned(),
        interface: HOME_SERVER_TUNNEL_NAME.to_owned(),
        role: TunnelRole::Server { listen_port: 51820 },
        origin: TunnelOrigin::NetworkManager {
            settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/15"),
            activity: TunnelActivity::Active {
                active_connection: object_path(
                    "/org/freedesktop/NetworkManager/ActiveConnection/10",
                ),
            },
        },
        statistics_access: StatisticsAccess::Read,
        peers: vec![
            wireguard_tunnels::Peer {
                public_key: "laptop-key".to_owned(),
                endpoint: PeerEndpoint::At("198.51.100.7:40211".to_owned()),
                allowed_ips: vec!["10.8.0.2/32".to_owned()],
                traffic: exchanged_traffic(42, ByteSize::mib(12), ByteSize::mib(3)),
            },
            wireguard_tunnels::Peer {
                public_key: "phone-key".to_owned(),
                endpoint: PeerEndpoint::Roaming,
                allowed_ips: vec!["10.8.0.3/32".to_owned()],
                traffic: PeerTraffic::NeverHandshaken,
            },
        ],
    }
}

pub fn on_hover(target: &WireguardTunnelTarget) -> Vec<WireguardTunnelsMessage> {
    let tunnel_ip = match target.interface.as_ref() {
        HOME_SERVER_TUNNEL_NAME => "10.8.0.1/24",
        LAB_TUNNEL_NAME => "10.42.0.7/24",
        _ => return Vec::new(),
    };
    vec![WireguardTunnelsMessage::TunnelAddressFetched(
        SnapshotDelivery::single_read(Ok(TunnelAddress {
            ip: Some(tunnel_ip.to_owned()),
        })),
    )]
}
