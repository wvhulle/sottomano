use chrono::TimeDelta;
use sottomano_sources::{
    SourceDelivery,
    network_connectivity::{
        interface_throughput::{InterfaceThroughput, NetworkTrafficDirection, Throughput},
        network_manager_proxies::Connectivity,
        tailscale_status::{ExitRouting, KeyExpiry, Peer, PeerPath, Tailnet, Tailscale},
    },
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::internet_group::{InternetGroup, InternetMessage},
};
use uom::si::{f64::InformationRate, information_rate::megabyte_per_second};

use crate::showcased_group::ShowcasedGroup;

const KEY_EXPIRY_DAYS: i64 = 9;

pub fn group(clock: Clock) -> ShowcasedGroup {
    ShowcasedGroup::Internet(Box::new(internet_readings(clock).into_iter().fold(
        InternetGroup::default(),
        |mut group, reading| {
            let _ = group.update(reading, clock);
            group
        },
    )))
}

fn internet_readings(clock: Clock) -> [InternetMessage; 2] {
    [
        InternetMessage::Throughput(SourceDelivery::Reading(Ok(InterfaceThroughput {
            down: wifi_throughput(NetworkTrafficDirection::Download, 4.2, 11.8),
            up: wifi_throughput(NetworkTrafficDirection::Upload, 0.4, 2.1),
        }))),
        InternetMessage::Tailscale(SourceDelivery::Reading(Ok(Tailscale::Running(Box::new(
            home_tailnet(clock),
        ))))),
    ]
}

fn wifi_throughput(
    direction: NetworkTrafficDirection,
    megabytes_per_second: f64,
    peak_megabytes_per_second: f64,
) -> Throughput {
    Throughput {
        interfaces: vec!["wlp4s0".to_owned()],
        rate: InformationRate::new::<megabyte_per_second>(megabytes_per_second),
        peak_rate: InformationRate::new::<megabyte_per_second>(peak_megabytes_per_second),
        connectivity: Some(Connectivity::Full),
        direction,
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
