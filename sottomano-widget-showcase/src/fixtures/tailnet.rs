use std::time::Duration;

use bytesize::ByteSize;
use chrono::TimeDelta;
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::{DeliveryCadence, PeriodicPolling},
    network_connectivity::{
        tailscale_inbound_access::{
            InboundAccessRule, InboundPorts, InboundReach, TailscaleInboundAccess,
        },
        tailscale_status::{ExitNodeOffer, KeyExpiry, Peer, PeerPath, Tailnet, Tailscale},
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::tailnet_group::{TailnetGroup, TailnetMessage, TailnetTarget},
};

use crate::showcased_group::ShowcasedGroup;

const KEY_EXPIRY_DAYS: i64 = 9;

pub fn group(clock: Clock) -> ShowcasedGroup {
    let mut tailnet_group = TailnetGroup::without_policy_file();
    let _ = tailnet_group.update(
        TailnetMessage::Tailscale(SnapshotDelivery::fixture_delivered(
            Ok(Tailscale::Running(Box::new(home_tailnet(clock)))),
            DeliveryCadence::PeriodicPolling(PeriodicPolling::every(Duration::from_secs(10))),
        )),
        clock,
    );
    ShowcasedGroup::Tailnet(Box::new(tailnet_group))
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

fn home_tailnet(clock: Clock) -> Tailnet {
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
        own_capabilities: vec!["file-sharing".to_owned(), "ssh".to_owned()],
        exit_offer: ExitNodeOffer::NotOffered,
        exit_node_in_use: true,
        key_expiry: KeyExpiry::At(clock.utc() + TimeDelta::days(KEY_EXPIRY_DAYS)),
        health_warnings: Vec::new(),
    }
}

pub fn on_hover(target: TailnetTarget) -> Vec<TailnetMessage> {
    match target {
        TailnetTarget::Membership
        | TailnetTarget::OwnNode
        | TailnetTarget::PolicyFile
        | TailnetTarget::ExitOffer => Vec::new(),
        TailnetTarget::InboundAccess => vec![TailnetMessage::InboundAccessFetched(
            SnapshotDelivery::single_read(Ok(TailscaleInboundAccess {
                rules: vec![
                    InboundAccessRule {
                        source_name: "home-server".to_owned(),
                        reach: InboundReach::ThisMachine,
                        ports: InboundPorts::All,
                    },
                    InboundAccessRule {
                        source_name: "office-laptop".to_owned(),
                        reach: InboundReach::ThisMachine,
                        ports: InboundPorts::Single(443),
                    },
                    InboundAccessRule {
                        source_name: "pixel-9".to_owned(),
                        reach: InboundReach::RoutesThroughThisMachine,
                        ports: InboundPorts::All,
                    },
                ],
            })),
        )],
    }
}
