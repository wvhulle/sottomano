use std::collections::HashMap;

use super::{
    wireguard_kernel_dumps::{
        KernelTunnel, KernelTunnelStatus, parse_kernel_dump, wireguard_interface_names,
    },
    wireguard_tunnel_assembly::assemble_tunnels,
    *,
};
use crate::network_connectivity::network_manager_proxies::{
    ActivationState, ActiveWireguard, WireguardProfile,
};

const NOW: u64 = 1_000_000;

const SERVER_DUMP_ROWS: &[&[&str]] = &[
    &["private", "public", "51820", "off"],
    &[
        "laptop-key",
        "(none)",
        "198.51.100.7:40211",
        "172.20.10.2/32",
        "999958",
        "12582912",
        "3145728",
        "off",
    ],
    &[
        "phone-key",
        "(none)",
        "(none)",
        "172.20.10.3/32",
        "0",
        "0",
        "0",
        "off",
    ],
];

const CLIENT_DUMP_ROWS: &[&[&str]] = &[
    &["private", "public", "43122", "off"],
    &[
        "server-key",
        "(none)",
        "203.0.113.24:51820",
        "0.0.0.0/0,::/0",
        "999963",
        "88080384",
        "12582912",
        "25",
    ],
];

fn dump_text(dump_rows: &[&[&str]]) -> String {
    dump_rows
        .iter()
        .map(|dump_row| dump_row.join("\t") + "\n")
        .collect()
}

fn parsed_dump(dump_rows: &[&[&str]]) -> KernelTunnel {
    parse_kernel_dump(&dump_text(dump_rows), NOW).unwrap()
}

fn object_path(path: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(path).unwrap()
}

fn host_peer_settings(public_key: &str, allowed_ip: &str) -> WireguardPeerSettings {
    WireguardPeerSettings {
        public_key: public_key.to_owned(),
        endpoint: PeerEndpoint::Roaming,
        allowed_ips: vec![allowed_ip.to_owned()],
    }
}

fn server_profile() -> WireguardProfile {
    WireguardProfile {
        settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/3"),
        name: "wg-wss".to_owned(),
        interface: "wg-wss".to_owned(),
        listen_port: 51820,
        peers: vec![
            host_peer_settings("laptop-key", "172.20.10.2/32"),
            host_peer_settings("phone-key", "172.20.10.3/32"),
        ],
    }
}

fn active_server() -> ActiveWireguard {
    ActiveWireguard {
        settings_connection: object_path("/org/freedesktop/NetworkManager/Settings/3"),
        active_connection: object_path("/org/freedesktop/NetworkManager/ActiveConnection/9"),
        activation_state: ActivationState::Activated,
    }
}

fn dumped(dump_rows: &[&[&str]]) -> KernelTunnelStatus {
    KernelTunnelStatus::Dumped(parsed_dump(dump_rows))
}

#[test]
fn configured_listen_port_decides_role() {
    assert_eq!(
        TunnelRole::configured(51820),
        TunnelRole::Server { listen_port: 51820 }
    );
    assert_eq!(TunnelRole::configured(0), TunnelRole::Client);
}

#[test]
fn single_peer_routing_everything_is_a_client() {
    let kernel_tunnel = parsed_dump(CLIENT_DUMP_ROWS);
    assert_eq!(
        TunnelRole::inferred_from_peers(kernel_tunnel.listen_port, &kernel_tunnel.peers),
        TunnelRole::Client
    );
}

#[test]
fn host_route_peers_make_a_server() {
    let kernel_tunnel = parsed_dump(SERVER_DUMP_ROWS);
    assert_eq!(
        TunnelRole::inferred_from_peers(kernel_tunnel.listen_port, &kernel_tunnel.peers),
        TunnelRole::Server { listen_port: 51820 }
    );
}

#[test]
fn dump_parses_peer_traffic() {
    let kernel_tunnel = parsed_dump(SERVER_DUMP_ROWS);
    assert_eq!(
        kernel_tunnel.peers[0].traffic,
        PeerTraffic::Exchanged(PeerStatistics {
            handshake_ago: Duration::from_secs(42),
            received: ByteSize::mib(12),
            sent: ByteSize::mib(3),
        })
    );
    assert_eq!(kernel_tunnel.peers[1].traffic, PeerTraffic::NeverHandshaken);
    assert_eq!(kernel_tunnel.peers[1].endpoint, PeerEndpoint::Roaming);
}

#[test]
fn unprivileged_profile_keeps_configured_peers_unobserved() {
    let wireguard = assemble_tunnels(
        vec![server_profile()],
        &[active_server()],
        HashMap::from([(
            "wg-wss".to_owned(),
            KernelTunnelStatus::Unreadable(StatisticsAccess::NeedsNetAdmin),
        )]),
    );
    let [tunnel] = wireguard.tunnels.as_slice() else {
        panic!("expected only the NetworkManager tunnel");
    };
    assert_eq!(tunnel.role, TunnelRole::Server { listen_port: 51820 });
    assert_eq!(tunnel.statistics_access, StatisticsAccess::NeedsNetAdmin);
    assert_eq!(tunnel.tunnel_phase(), LinkPhase::Up);
    assert_eq!(tunnel.peers.len(), 2);
    assert!(
        tunnel
            .peers
            .iter()
            .all(|peer| peer.traffic == PeerTraffic::Unobserved)
    );
    assert_eq!(
        tunnel.switch(),
        Some(NmConnectionSwitch::Disconnect(object_path(
            "/org/freedesktop/NetworkManager/ActiveConnection/9"
        )))
    );
}

#[test]
fn readable_dump_fills_profile_peer_traffic() {
    let wireguard = assemble_tunnels(
        vec![server_profile()],
        &[active_server()],
        HashMap::from([("wg-wss".to_owned(), dumped(SERVER_DUMP_ROWS))]),
    );
    let tunnel = &wireguard.tunnels[0];
    assert_eq!(tunnel.connected_peers(), 1);
    assert_eq!(
        tunnel.peers[0].endpoint,
        PeerEndpoint::At("198.51.100.7:40211".to_owned())
    );
}

#[test]
fn inactive_profile_switches_to_connect() {
    let wireguard = assemble_tunnels(vec![server_profile()], &[], HashMap::new());
    let tunnel = &wireguard.tunnels[0];
    assert_eq!(tunnel.tunnel_phase(), LinkPhase::Down);
    assert_eq!(tunnel.statistics_access, StatisticsAccess::TunnelDown);
    assert_eq!(
        tunnel.switch(),
        Some(NmConnectionSwitch::Connect(object_path(
            "/org/freedesktop/NetworkManager/Settings/3"
        )))
    );
}

#[test]
fn kernel_only_tunnel_has_no_switch() {
    let wireguard = assemble_tunnels(
        Vec::new(),
        &[],
        HashMap::from([("wg0".to_owned(), dumped(CLIENT_DUMP_ROWS))]),
    );
    let tunnel = &wireguard.tunnels[0];
    assert_eq!(tunnel.origin, TunnelOrigin::KernelOnly);
    assert_eq!(tunnel.switch(), None);
    assert_eq!(tunnel.traffic_routing(), TunnelRouting::DefaultRoute);
}

#[test]
fn ip_link_listing_yields_interface_names() {
    let listing = r#"[{"ifindex":7,"ifname":"wg-wss","flags":["POINTOPOINT","NOARP","UP"]}]"#;
    assert_eq!(wireguard_interface_names(listing), ["wg-wss"]);
}
