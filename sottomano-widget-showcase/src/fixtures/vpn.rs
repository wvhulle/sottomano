use chrono::TimeDelta;
use sottomano_sources::{
    SourceDelivery,
    network_connectivity::{
        network_manager_proxies::AddressBlock,
        network_manager_vpn_connections::{
            ActiveTunnel, NetworkManagerVpns, VpnActivity, VpnConnection, VpnState,
        },
        tailscale_status::{ExitRouting, KeyExpiry, Peer, PeerPath, Tailnet, Tailscale},
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::vpn_group::{VpnGroup, VpnMessage},
};
use zbus::zvariant::OwnedObjectPath;

use crate::showcased_group::ShowcasedGroup;

const KEY_EXPIRY_DAYS: i64 = 9;

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

fn vpn_readings(clock: Clock) -> [VpnMessage; 2] {
    [
        VpnMessage::NetworkManagerVpns(SourceDelivery::Reading(Ok(NetworkManagerVpns {
            connections: vec![company_globalprotect_vpn()],
        }))),
        VpnMessage::Tailscale(SourceDelivery::Reading(Ok(Tailscale::Running(Box::new(
            home_tailnet(clock),
        ))))),
    ]
}

fn company_globalprotect_vpn() -> VpnConnection {
    VpnConnection {
        name: COMPANY_VPN_NAME.to_owned(),
        plugin: "openconnect".to_owned(),
        protocol: Some("gp".to_owned()),
        remote_gateway: Some("vpn.example.com".to_owned()),
        settings_connection: OwnedObjectPath::try_from(
            "/org/freedesktop/NetworkManager/Settings/12",
        )
        .expect("static object path"),
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
            active_connection: OwnedObjectPath::try_from(
                "/org/freedesktop/NetworkManager/ActiveConnection/7",
            )
            .expect("static object path"),
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
