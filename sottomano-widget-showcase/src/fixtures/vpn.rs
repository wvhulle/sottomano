use std::time::Duration;

use bytesize::ByteSize;
use chrono::TimeDelta;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    network_connectivity::{
        network_manager_proxies::AddressBlock,
        network_manager_vpn_connections::{
            ActiveTunnel, NetworkManagerVpns, VpnActivity, VpnConnection, VpnState,
        },
        tailscale_exit_nodes::{
            ExitNodeChoices, ExitNodeLanAccess, ExitNodeOption, ExitNodeReachability,
            TailscaleExitNodes,
        },
        tailscale_status::ExitNodeOffer,
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

const OFFLINE_EXIT_NODE_DAYS: i64 = 14;
pub const HOME_EXIT_NODE_IP: &str = "100.88.40.2";
pub const LAB_TUNNEL_NAME: &str = "wg-lab";

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
        VpnMessage::TailscaleExitNodes(SnapshotDelivery::fixture_delivered(
            Ok(TailscaleExitNodes::Running(home_exit_nodes(clock))),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(10))),
        )),
        VpnMessage::Wireguard(SnapshotDelivery::fixture_delivered(
            Ok(Wireguard {
                tunnels: vec![lab_wireguard_tunnel()],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
    ]
}

pub(super) fn object_path(path: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(path).expect("static object path")
}

pub(super) fn exchanged_traffic(
    handshake_seconds: u64,
    received: ByteSize,
    sent: ByteSize,
) -> PeerTraffic {
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

fn home_exit_nodes(clock: Clock) -> ExitNodeChoices {
    ExitNodeChoices {
        options: vec![
            ExitNodeOption {
                name: "home-server".to_owned(),
                ip: HOME_EXIT_NODE_IP.to_owned(),
                reachability: ExitNodeReachability::Online,
                in_use: true,
            },
            ExitNodeOption {
                name: "office-laptop".to_owned(),
                ip: "100.70.3.9".to_owned(),
                reachability: ExitNodeReachability::OfflineSince(
                    clock.utc() - TimeDelta::days(OFFLINE_EXIT_NODE_DAYS),
                ),
                in_use: false,
            },
        ],
        offer: ExitNodeOffer::NotOffered,
        lan_access: ExitNodeLanAccess::Allowed,
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
        VpnTarget::Wireguard(_)
        | VpnTarget::NetworkManagerVpn(_)
        | VpnTarget::TailscaleExitNode(_)
        | VpnTarget::TailscaleLanAccess
        | VpnTarget::TailscaleExitNodesFailure => Vec::new(),
    }
}
