use std::{
    collections::HashMap,
    fmt::{Display, Formatter, Result as FmtResult},
    time::Duration,
};

use chrono::{DateTime, Utc};
use futures::{Stream, stream};
use serde::Deserialize;

use crate::{
    DeliveryCadence, ReadingSource, SourceDelivery, polled_delivery_stream,
    stream_operators::StreamOperators,
    system_protocol_clients::{shell_command_output, shell_command_output::blocking::run},
};

const RUNNING_BACKEND_STATE: &str = "Running";
pub const ADMIN_CONSOLE_URL: &str = "https://login.tailscale.com/admin/machines";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Command(shell_command_output::Error),
    Malformed,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Error::Command(error) => write!(f, "{error}"),
            Error::Malformed => write!(f, "unexpected status JSON"),
        }
    }
}

impl From<shell_command_output::Error> for Error {
    fn from(error: shell_command_output::Error) -> Self {
        Error::Command(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerPath {
    Direct,
    Relayed { region: String },
    Idle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub name: String,
    pub ip: String,
    pub os: String,
    pub path: PeerPath,
    pub received_bytes: u64,
    pub sent_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitRouting {
    Local,
    ThroughPeer { peer_name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyExpiry {
    Disabled,
    At(DateTime<Utc>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tailnet {
    pub tailnet_name: String,
    pub own_node: Peer,
    pub online: Vec<Peer>,
    pub total_peers: usize,
    pub exit_routing: ExitRouting,
    pub key_expiry: KeyExpiry,
    pub health_warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tailscale {
    Stopped,
    Running(Box<Tailnet>),
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Status {
    backend_state: String,
    #[serde(rename = "Self")]
    own_node: Option<Node>,
    #[serde(default)]
    peer: HashMap<String, Node>,
    current_tailnet: Option<CurrentTailnet>,
    #[serde(default)]
    health: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CurrentTailnet {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Node {
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Option<Vec<String>>,
    #[serde(default)]
    online: bool,
    #[serde(rename = "OS", default)]
    os: String,
    #[serde(default)]
    cur_addr: String,
    #[serde(default)]
    relay: String,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    rx_bytes: u64,
    #[serde(default)]
    tx_bytes: u64,
    #[serde(rename = "ExitNode", default)]
    carries_exit_traffic: bool,
    #[serde(default)]
    key_expiry: Option<DateTime<Utc>>,
}

impl Node {
    fn short_name(&self) -> String {
        self.dns_name
            .split('.')
            .next()
            .unwrap_or(&self.dns_name)
            .to_owned()
    }

    fn peer_path(&self) -> PeerPath {
        if !self.active {
            PeerPath::Idle
        } else if self.cur_addr.is_empty() {
            PeerPath::Relayed {
                region: self.relay.clone(),
            }
        } else {
            PeerPath::Direct
        }
    }
}

impl TryFrom<&Node> for Peer {
    type Error = Error;

    fn try_from(node: &Node) -> Result<Self, Error> {
        let ip = node
            .tailscale_ips
            .as_deref()
            .and_then(<[String]>::first)
            .ok_or(Error::Malformed)?;
        Ok(Peer {
            name: node.short_name(),
            ip: ip.clone(),
            os: node.os.clone(),
            path: node.peer_path(),
            received_bytes: node.rx_bytes,
            sent_bytes: node.tx_bytes,
        })
    }
}

impl ReadingSource for Tailscale {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "tailscale";
    const DELIVERY_CADENCE: DeliveryCadence = DeliveryCadence::PolledEvery(Duration::from_secs(10));

    fn delivery_stream(inputs: ()) -> impl Stream<Item = SourceDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("tailscale")).while_held(
            move || polled_delivery_stream(inputs, Self::poll),
            SourceDelivery::Absent,
        )
    }
}

impl Tailscale {
    async fn poll(_key: ()) -> Result<Self, Error> {
        let raw = shell_command_output::output("tailscale", &["status", "--json"]).await?;
        Self::from_status_json(&raw)
    }

    fn from_status_json(raw: &str) -> Result<Self, Error> {
        let status: Status = serde_json::from_str(raw).map_err(|_| Error::Malformed)?;
        if status.backend_state != RUNNING_BACKEND_STATE {
            return Ok(Tailscale::Stopped);
        }
        let own_node = status.own_node.as_ref().ok_or(Error::Malformed)?;
        let tailnet_name = status.current_tailnet.ok_or(Error::Malformed)?.name;
        let exit_routing = status
            .peer
            .values()
            .find(|peer| peer.carries_exit_traffic)
            .map_or(ExitRouting::Local, |peer| ExitRouting::ThroughPeer {
                peer_name: peer.short_name(),
            });
        let mut online: Vec<Peer> = status
            .peer
            .values()
            .filter(|peer| peer.online)
            .map(Peer::try_from)
            .filter_map(Result::ok)
            .collect();
        online.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Tailscale::Running(Box::new(Tailnet {
            tailnet_name,
            own_node: Peer::try_from(own_node)?,
            online,
            total_peers: status.peer.len(),
            exit_routing,
            key_expiry: own_node
                .key_expiry
                .map_or(KeyExpiry::Disabled, KeyExpiry::At),
            health_warnings: status.health.unwrap_or_default(),
        })))
    }
}

#[must_use]
pub fn open_admin() -> bool {
    run("xdg-open", &[ADMIN_CONSOLE_URL])
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS_FIXTURE: &str = r#"{
        "BackendState": "Running",
        "Health": null,
        "CurrentTailnet": {"Name": "example.com"},
        "Self": {
            "DNSName": "ryzen.tail.ts.net.",
            "TailscaleIPs": ["100.95.140.64"],
            "OS": "linux",
            "Online": true,
            "KeyExpiry": "2026-12-08T19:22:27Z"
        },
        "Peer": {
            "a": {"DNSName": "zbook.tail.ts.net.", "TailscaleIPs": ["100.1.1.1"], "OS": "linux",
                  "Online": true, "Active": true, "CurAddr": "1.2.3.4:41641", "Relay": "ams", "ExitNode": true},
            "b": {"DNSName": "phone.tail.ts.net.", "TailscaleIPs": ["100.1.1.2"], "OS": "iOS",
                  "Online": true, "Active": true, "CurAddr": "", "Relay": "tok"},
            "c": {"DNSName": "macbook.tail.ts.net.", "TailscaleIPs": ["100.1.1.3"], "OS": "macOS",
                  "Online": true, "Active": false, "Relay": "ams"},
            "d": {"DNSName": "x1.tail.ts.net.", "TailscaleIPs": ["100.1.1.4"], "OS": "linux", "Online": false}
        }
    }"#;

    fn running_tailnet() -> Tailnet {
        match Tailscale::from_status_json(STATUS_FIXTURE) {
            Ok(Tailscale::Running(tailnet)) => *tailnet,
            other => panic!("expected running tailnet, got {other:?}"),
        }
    }

    #[test]
    fn online_peers_are_sorted_and_classified_by_path() {
        let tailnet = running_tailnet();
        let names_and_paths: Vec<_> = tailnet
            .online
            .iter()
            .map(|peer| (peer.name.as_str(), peer.path.clone()))
            .collect();
        assert_eq!(
            names_and_paths,
            [
                ("macbook", PeerPath::Idle),
                (
                    "phone",
                    PeerPath::Relayed {
                        region: "tok".to_owned()
                    }
                ),
                ("zbook", PeerPath::Direct),
            ]
        );
        assert_eq!(tailnet.total_peers, 4);
    }

    #[test]
    fn exit_node_tailnet_and_key_expiry_are_read() {
        let tailnet = running_tailnet();
        assert_eq!(
            tailnet.exit_routing,
            ExitRouting::ThroughPeer {
                peer_name: "zbook".to_owned()
            }
        );
        assert_eq!(tailnet.tailnet_name, "example.com");
        assert!(matches!(tailnet.key_expiry, KeyExpiry::At(_)));
        assert!(tailnet.health_warnings.is_empty());
    }
}
