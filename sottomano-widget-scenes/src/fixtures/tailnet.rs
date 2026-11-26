use std::{collections::BTreeMap, time::Duration};

use bytesize::ByteSize;
use chrono::TimeDelta;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    network_connectivity::{
        tailscale_exit_nodes::{
            ExitNodeChoices, ExitNodeLanAccess, ExitNodeOption, ExitNodeReachability,
            ExitRoutingSwitch, TailscaleExitNodes,
        },
        tailscale_inbound_access::{
            InboundAccessRule, InboundDestination, InboundPorts, TailscaleInboundAccess,
        },
        tailscale_status::{
            ExitNodeOffer, ExitNodeUse, KeyExpiry, NodeCapabilities, Peer, PeerPath, Tailnet,
            Tailscale,
        },
    },
};
use sottomano_widgets::{
    clock::Clock,
    panel_menus::network_menu::tailnet_group::{TailnetGroup, TailnetInteraction, TailnetMessage},
};

use crate::showcased_group::ShowcasedGroup;

const KEY_EXPIRY_DAYS: i64 = 9;
const OFFLINE_EXIT_NODE_DAYS: i64 = 14;
pub const HOME_EXIT_NODE_IP: &str = "100.88.40.2";

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut tailnet_group = TailnetGroup::without_policy_file();
    for routed_message in routed_through_home_server(ExitNodeUse::InUse, clock) {
        let _ = tailnet_group.update(routed_message, clock);
    }
    ShowcasedGroup::Tailnet(Box::new(tailnet_group))
}

fn routed_through_home_server(home_server_use: ExitNodeUse, clock: Clock) -> [TailnetMessage; 2] {
    [
        TailnetMessage::Tailscale(SnapshotDelivery::fixture_delivered(
            Ok(Tailscale::Running(Box::new(home_tailnet(
                home_server_use,
                clock,
            )))),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(10))),
        )),
        TailnetMessage::ExitNodesDelivered(SnapshotDelivery::fixture_delivered(
            Ok(TailscaleExitNodes::Running(home_exit_nodes(
                home_server_use,
                clock,
            ))),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(10))),
        )),
    ]
}

pub fn on_press(pressed_switch: &TailnetInteraction, clock: Clock) -> Vec<TailnetMessage> {
    let home_server_use = match pressed_switch {
        TailnetInteraction::SwitchExitRouting(ExitRoutingSwitch::StopUsingExitNode) => {
            ExitNodeUse::Unused
        }
        TailnetInteraction::SwitchExitRouting(ExitRoutingSwitch::UseExitNode { ip })
            if ip == HOME_EXIT_NODE_IP =>
        {
            ExitNodeUse::InUse
        }
        _ => return Vec::new(),
    };
    routed_through_home_server(home_server_use, clock).into()
}

fn tailnet_peer(
    name: &str,
    ip: &str,
    os: &str,
    path: PeerPath,
    received_megabytes: u64,
    tags: &[&str],
) -> Peer {
    Peer {
        name: name.to_owned(),
        ip: ip.to_owned(),
        os: os.to_owned(),
        path,
        received_bytes: ByteSize::mib(received_megabytes).as_u64(),
        sent_bytes: ByteSize::mib(received_megabytes / 3).as_u64(),
        tags: tags.iter().map(ToString::to_string).collect(),
    }
}

fn home_tailnet(home_server_use: ExitNodeUse, clock: Clock) -> Tailnet {
    Tailnet {
        tailnet_name: "example.ts.net".to_owned(),
        own_node: tailnet_peer(
            "workstation",
            "100.101.12.4",
            "linux",
            PeerPath::Idle,
            0,
            &["tag:home"],
        ),
        online: vec![
            tailnet_peer(
                "home-server",
                "100.88.40.2",
                "linux",
                PeerPath::Direct,
                412,
                &["tag:home"],
            ),
            tailnet_peer(
                "pixel-9",
                "100.92.7.31",
                "android",
                PeerPath::Relayed {
                    region: "ams".to_owned(),
                },
                18,
                &[],
            ),
            tailnet_peer(
                "office-laptop",
                "100.70.3.9",
                "macOS",
                PeerPath::Idle,
                0,
                &["tag:work"],
            ),
        ],
        total_peers: 5,
        own_capabilities: NodeCapabilities {
            unscoped_features: [
                "file-sharing",
                "https",
                "ssh",
                "ssh-env-vars",
                "tailnet-lock",
            ]
            .map(str::to_owned)
            .to_vec(),
            scoped_by_namespace: BTreeMap::from([(
                "services".to_owned(),
                ["jellyfin", "nextcloud", "vaultwarden"]
                    .map(str::to_owned)
                    .to_vec(),
            )]),
        },
        exit_offer: ExitNodeOffer::NotOffered,
        exit_node_use: home_server_use,
        key_expiry: KeyExpiry::At(clock.utc() + TimeDelta::days(KEY_EXPIRY_DAYS)),
        health_warnings: Vec::new(),
    }
}

fn home_exit_nodes(home_server_use: ExitNodeUse, clock: Clock) -> ExitNodeChoices {
    ExitNodeChoices {
        options: vec![
            ExitNodeOption {
                name: "home-server".to_owned(),
                ip: HOME_EXIT_NODE_IP.to_owned(),
                reachability: ExitNodeReachability::Online,
                node_use: home_server_use,
            },
            ExitNodeOption {
                name: "office-laptop".to_owned(),
                ip: "100.70.3.9".to_owned(),
                reachability: ExitNodeReachability::OfflineSince(
                    clock.utc() - TimeDelta::days(OFFLINE_EXIT_NODE_DAYS),
                ),
                node_use: ExitNodeUse::Unused,
            },
        ],
        offer: ExitNodeOffer::NotOffered,
        lan_access: ExitNodeLanAccess::Allowed,
    }
}

pub fn on_hover(started_fetch: &TailnetInteraction) -> Vec<TailnetMessage> {
    match started_fetch {
        TailnetInteraction::StartInboundAccessFetch => vec![TailnetMessage::InboundAccessFetched(
            SnapshotDelivery::single_read(Ok(TailscaleInboundAccess {
                rules: vec![
                    InboundAccessRule {
                        source_name: "home-server".to_owned(),
                        destination: InboundDestination::ThisMachine,
                        ports: InboundPorts::All,
                    },
                    InboundAccessRule {
                        source_name: "office-laptop".to_owned(),
                        destination: InboundDestination::ThisMachine,
                        ports: InboundPorts::Single(443),
                    },
                    InboundAccessRule {
                        source_name: "office-laptop".to_owned(),
                        destination: InboundDestination::HostedService {
                            service_name: "nextcloud".to_owned(),
                        },
                        ports: InboundPorts::Single(443),
                    },
                    InboundAccessRule {
                        source_name: "pixel-9".to_owned(),
                        destination: InboundDestination::ExitNodeInternet,
                        ports: InboundPorts::All,
                    },
                ],
            })),
        )],
        _ => Vec::new(),
    }
}
