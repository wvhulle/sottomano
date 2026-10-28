use std::time::Duration;

use bytesize::ByteSize;
use chrono::TimeDelta;
use sottomano_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    network_connectivity::{
        network_manager_proxies::AddressBlock,
        network_manager_vpn_connections::{
            ActiveTunnel, NetworkManagerVpns, VpnActivity, VpnConnection, VpnState,
        },
        tailscale_status::{ExitRouting, KeyExpiry, Peer, PeerPath, Tailnet, Tailscale},
        wireguard_tunnels::{
            self, PeerEndpoint, PeerStatistics, PeerTraffic, StatisticsAccess, Tunnel,
            TunnelActivity, TunnelAddress, TunnelOrigin, TunnelRole, Wireguard,
        },
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::vpn_group::{VpnGroup, VpnMessage, VpnTarget},
};
use zbus::zvariant::OwnedObjectPath;

use crate::showcased_group::ShowcasedGroup;

const KEY_EXPIRY_DAYS: i64 = 9;
pub const LAB_TUNNEL_NAME: &str = "wg-lab";
pub const HOME_SERVER_TUNNEL_NAME: &str = "wg-home";

pub const COMPANY_VPN_NAME: &str = "Company VPN";

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::Vpn(Box::new(vpn_readings(clock).into_iter().fold(
        VpnGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading, clock);
            group
        },
    )))
}

fn vpn_readings(clock: Clock) -> [VpnMessage; 3] {
    [
        VpnMessage::NetworkManagerVpns(SnapshotDelivery::live_push(Ok(NetworkManagerVpns {
            connections: vec![company_globalprotect_vpn()],
        }))),
        VpnMessage::Tailscale(SnapshotDelivery::fixture_delivered(
            Ok(Tailscale::Running(Box::new(home_tailnet(clock)))),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(10))),
        )),
        VpnMessage::Wireguard(SnapshotDelivery::fixture_delivered(
            Ok(Wireguard {
                tunnels: vec![lab_wireguard_tunnel(), home_wireguard_server()],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
    ]
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

fn company_globalprotect_vpn() -> VpnConnection {
    VpnConnection {
        name: COMPANY_VPN_NAME.to_owned(),
        plugin: "openconnect".to_owned(),
        protocol: Some("gp".to_owned()),
        remote_gateway: Some("vpn.example.com".to_owned()),
        settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/12"),
        activity: VpnActivity::Active(Box::new(ActiveTunnel {
            vpn_state: VpnState::Activated,
            ipv4: Some(AddressBlock {
                address: "10.84.17.203".to_owned(),
                mask: "255.255.255.255".to_owned(),
                gateway: None,
            }),
            ipv6: None,
            dns: vec!["10.80.0.53".to_owned(), "10.80.0.54".to_owned()],
            banner: None,
            active_connection: object_path("/org/freedesktop/NetworkManager/ActiveConnection/7"),
        })),
    }
}

fn tailnet_peer(name: &str, ip: &str, os: &str, path: PeerPath, mebibytes: u64) -> Peer {
    Peer {
        name: name.to_owned(),
        ip: ip.to_owned(),
        os: os.to_owned(),
        path,
        received_bytes: mebibytes * 1024 * 1024,
        sent_bytes: mebibytes * 300 * 1024,
    }
}

fn home_tailnet(clock: Clock) -> Tailnet {
    Tailnet {
        tailnet_name: "example.ts.net".to_owned(),
        own_node: tailnet_peer("workstation", "100.101.12.4", "linux", PeerPath::Idle, 0),
        online: vec![
            tailnet_peer("home-server", "100.88.40.2", "linux", PeerPath::Direct, 412),
            tailnet_peer(
                "pixel-9",
                "100.92.7.31",
                "android",
                PeerPath::Relayed {
                    region: "ams".to_owned(),
                },
                18,
            ),
            tailnet_peer("office-laptop", "100.70.3.9", "macOS", PeerPath::Idle, 0),
        ],
        total_peers: 5,
        exit_routing: ExitRouting::ThroughPeer {
            peer_name: "home-server".to_owned(),
        },
        key_expiry: KeyExpiry::At(clock.utc() + TimeDelta::days(KEY_EXPIRY_DAYS)),
        health_warnings: Vec::new(),
    }
}

pub fn on_hover(target: &VpnTarget) -> Vec<VpnMessage> {
    match target {
        VpnTarget::Wireguard(tunnel) if tunnel.as_ref() == LAB_TUNNEL_NAME => {
            vec![VpnMessage::TunnelAddressFetched(
                SnapshotDelivery::single_read(Ok(TunnelAddress {
                    ip: Some("10.42.0.7/24".to_owned()),
                })),
            )]
        }
        VpnTarget::Wireguard(tunnel) if tunnel.as_ref() == HOME_SERVER_TUNNEL_NAME => {
            vec![VpnMessage::TunnelAddressFetched(
                SnapshotDelivery::single_read(Ok(TunnelAddress {
                    ip: Some("10.8.0.1/24".to_owned()),
                })),
            )]
        }
        VpnTarget::Wireguard(_) | VpnTarget::NetworkManagerVpn(_) | VpnTarget::Tailscale => {
            Vec::new()
        }
    }
}
