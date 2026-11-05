use std::time::Duration;

use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    network_connectivity::wireguard_tunnels::{
        self, PeerEndpoint, PeerTraffic, StatisticsAccess, Tunnel, TunnelActivity, TunnelAddress,
        TunnelOrigin, TunnelRole, Wireguard,
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::wireguard_server_group::{WireguardServerGroup, WireguardServerMessage},
};

use crate::{
    fixtures::vpn::{exchanged_traffic, object_path},
    showcased_group::ShowcasedGroup,
};

pub const HOME_SERVER_TUNNEL_NAME: &str = "wg-home";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut server_group = WireguardServerGroup::default();
    let _ = server_group.update(
        WireguardServerMessage::Wireguard(SnapshotDelivery::fixture_delivered(
            Ok(Wireguard {
                tunnels: vec![home_wireguard_server()],
            }),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(5))),
        )),
        clock,
    );
    ShowcasedGroup::WireguardServer(Box::new(server_group))
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

pub fn on_hover() -> Vec<WireguardServerMessage> {
    vec![WireguardServerMessage::TunnelAddressFetched(
        SnapshotDelivery::single_read(Ok(TunnelAddress {
            ip: Some("10.8.0.1/24".to_owned()),
        })),
    )]
}
