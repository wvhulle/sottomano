use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytesize::ByteSize;
use serde_json::Value as Json;

use super::{Peer, PeerEndpoint, PeerStatistics, PeerTraffic, StatisticsAccess};
use crate::system_protocol_clients::shell_command_output;

const PERMISSION_DENIED_MESSAGE: &str = "Operation not permitted";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct KernelTunnel {
    pub(super) listen_port: u32,
    pub(super) peers: Vec<Peer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KernelTunnelStatus {
    Dumped(KernelTunnel),
    Unreadable(StatisticsAccess),
}

pub(super) async fn kernel_wireguard_interfaces() -> Vec<String> {
    match shell_command_output::output("ip", &["-j", "link", "show", "type", "wireguard"]).await {
        Ok(listing) => wireguard_interface_names(&listing),
        Err(listing_error) => {
            tracing::debug!("wireguard: cannot list kernel interfaces: {listing_error}");
            Vec::new()
        }
    }
}

pub(super) fn wireguard_interface_names(listing: &str) -> Vec<String> {
    serde_json::from_str::<Json>(listing)
        .inspect_err(|parse_failure| {
            tracing::warn!("wireguard: invalid kernel interface listing: {parse_failure}");
        })
        .ok()
        .and_then(|json| {
            json.as_array().map(|links| {
                links
                    .iter()
                    .filter_map(|link| link.get("ifname")?.as_str().map(ToOwned::to_owned))
                    .collect()
            })
        })
        .unwrap_or_default()
}

pub(super) async fn read_kernel_tunnel(interface: &str) -> KernelTunnelStatus {
    match shell_command_output::output("wg", &["show", interface, "dump"]).await {
        Ok(dump) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since_epoch| since_epoch.as_secs());
            parse_kernel_dump(&dump, now).map_or_else(
                || {
                    KernelTunnelStatus::Unreadable(StatisticsAccess::Unreadable(format!(
                        "empty dump for {interface}"
                    )))
                },
                KernelTunnelStatus::Dumped,
            )
        }
        Err(shell_command_output::Error::Failed { stderr, .. })
            if stderr.contains(PERMISSION_DENIED_MESSAGE) =>
        {
            KernelTunnelStatus::Unreadable(StatisticsAccess::NeedsNetAdmin)
        }
        Err(dump_error) => {
            tracing::debug!("wireguard: cannot dump {interface}: {dump_error}");
            KernelTunnelStatus::Unreadable(StatisticsAccess::Unreadable(dump_error.to_string()))
        }
    }
}

pub(super) fn parse_kernel_dump(dump: &str, now: u64) -> Option<KernelTunnel> {
    let mut lines = dump.lines();
    let interface_line = lines.next()?;
    let listen_port = interface_line
        .split('\t')
        .nth(2)
        .and_then(|port| {
            port.parse()
                .inspect_err(|parse_failure| {
                    tracing::warn!(
                        "wireguard: invalid listen port {port:?} in dump: {parse_failure}"
                    );
                })
                .ok()
        })
        .unwrap_or_default();
    Some(KernelTunnel {
        listen_port,
        peers: lines.filter_map(|line| kernel_peer(line, now)).collect(),
    })
}

#[expect(
    clippy::disallowed_methods,
    reason = "the kernel stamps handshakes with its own wall clock reading, which can be a moment \
              ahead of the dump's"
)]
fn kernel_peer(dump_line: &str, now: u64) -> Option<Peer> {
    let fields: Vec<&str> = dump_line.split('\t').collect();
    let reported = |position: usize| {
        fields
            .get(position)
            .filter(|field| !matches!(**field, "(none)" | ""))
            .map(ToString::to_string)
    };
    let handshake = dump_counter(&fields, 4)?;
    let received = ByteSize::b(dump_counter(&fields, 5)?);
    let sent = ByteSize::b(dump_counter(&fields, 6)?);
    Some(Peer {
        public_key: reported(0)?,
        endpoint: PeerEndpoint::from(reported(2)),
        allowed_ips: reported(3)
            .map(|allowed_ips| allowed_ips.split(',').map(ToOwned::to_owned).collect())
            .unwrap_or_default(),
        traffic: if handshake == 0 {
            PeerTraffic::NeverHandshaken
        } else {
            PeerTraffic::Exchanged(PeerStatistics {
                handshake_ago: Duration::from_secs(now.saturating_sub(handshake)),
                received,
                sent,
            })
        },
    })
}

fn dump_counter(fields: &[&str], position: usize) -> Option<u64> {
    let field = fields.get(position)?;
    field
        .parse()
        .inspect_err(|parse_failure| {
            tracing::warn!(
                "wireguard: invalid peer counter {field:?} in dump column {position}: \
                 {parse_failure}"
            );
        })
        .ok()
}
