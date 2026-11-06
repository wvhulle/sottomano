use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Datelike, Utc};
use futures::{Stream, stream};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::{
    SnapshotDelivery, SnapshotSource, delivery_cadence_drivers::PeriodicPolling,
    stream_operators::StreamOperators, system_protocol_clients::shell_command_output,
};

const RUNNING_BACKEND_STATE: &str = "Running";
const STARTING_BACKEND_STATE: &str = "Starting";
const INTERNET_ROUTES: [&str; 2] = ["0.0.0.0/0", "::/0"];
const FIRST_REAL_LAST_SEEN_YEAR: i32 = 2;
const CAPABILITY_URL_PREFIX: &str = "https://tailscale.com/cap/";
pub const ADMIN_CONSOLE_URL: &str = "https://login.tailscale.com/admin/machines";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Command(#[from] shell_command_output::Error),
    #[error("unexpected status JSON")]
    Malformed,
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
    pub tags: Vec<String>,
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
    pub own_capabilities: Vec<String>,
    pub exit_offer: ExitNodeOffer,
    pub exit_node_in_use: bool,
    pub key_expiry: KeyExpiry,
    pub health_warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tailscale {
    Stopped,
    Starting,
    Running(Box<Tailnet>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitNodeOffer {
    NotOffered,
    AwaitingApproval,
    Approved,
}

impl ExitNodeOffer {
    #[must_use]
    pub fn is_offered(self) -> bool {
        self != ExitNodeOffer::NotOffered
    }

    pub(super) fn from_status_and_prefs(
        status: &TailscaleStatusJson,
        prefs: &TailscalePrefsJson,
    ) -> Self {
        let approved = status
            .own_node
            .as_ref()
            .is_some_and(|own_node| own_node.offers_exit_node);
        match (prefs.advertises_exit_node(), approved) {
            (false, _) => ExitNodeOffer::NotOffered,
            (true, false) => ExitNodeOffer::AwaitingApproval,
            (true, true) => ExitNodeOffer::Approved,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitOfferSwitch {
    StartOffering,
    StopOffering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitOfferControl {
    Switchable(ExitOfferSwitch),
    BlockedByExitNodeInUse,
}

impl Tailnet {
    #[must_use]
    pub fn offer_control(&self) -> ExitOfferControl {
        if self.exit_offer.is_offered() {
            ExitOfferControl::Switchable(ExitOfferSwitch::StopOffering)
        } else if self.exit_node_in_use {
            ExitOfferControl::BlockedByExitNodeInUse
        } else {
            ExitOfferControl::Switchable(ExitOfferSwitch::StartOffering)
        }
    }
}

#[derive(Deserialize)]
pub(super) struct TailscalePrefsJson {
    #[serde(
        rename = "AdvertiseRoutes",
        default,
        deserialize_with = "null_as_empty"
    )]
    advertised_routes: Vec<String>,
    #[serde(rename = "ExitNodeAllowLANAccess", default)]
    pub(super) lan_access_allowed: bool,
}

impl TailscalePrefsJson {
    fn advertises_exit_node(&self) -> bool {
        self.advertised_routes
            .iter()
            .any(|route| INTERNET_ROUTES.contains(&route.as_str()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailscaleSwitch {
    Up,
    Down,
}

pub(super) fn null_as_empty<'de, Deserializing, Collection>(
    deserializer: Deserializing,
) -> Result<Collection, Deserializing::Error>
where
    Deserializing: Deserializer<'de>,
    Collection: Deserialize<'de> + Default,
{
    Ok(Option::<Collection>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(from = "Option<DateTime<Utc>>")]
pub(super) enum NodeLastSeen {
    #[default]
    Never,
    At(DateTime<Utc>),
}

impl From<Option<DateTime<Utc>>> for NodeLastSeen {
    fn from(last_seen: Option<DateTime<Utc>>) -> Self {
        last_seen
            .filter(|moment| moment.year() >= FIRST_REAL_LAST_SEEN_YEAR)
            .map_or(NodeLastSeen::Never, NodeLastSeen::At)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct TailscaleStatusJson {
    pub(super) backend_state: String,
    #[serde(rename = "Self")]
    pub(super) own_node: Option<TailscaleNodeJson>,
    #[serde(default)]
    pub(super) peer: HashMap<String, TailscaleNodeJson>,
    current_tailnet: Option<CurrentTailnetJson>,
    #[serde(default, deserialize_with = "null_as_empty")]
    health: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CurrentTailnetJson {
    name: String,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct TailscaleNodeJson {
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default, deserialize_with = "null_as_empty")]
    pub(super) tailscale_ips: Vec<String>,
    #[serde(default)]
    pub(super) online: bool,
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
    pub(super) carries_exit_traffic: bool,
    #[serde(rename = "ExitNodeOption", default)]
    pub(super) offers_exit_node: bool,
    #[serde(default)]
    pub(super) last_seen: NodeLastSeen,
    #[serde(default)]
    key_expiry: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "null_as_empty")]
    tags: Vec<String>,
    #[serde(default, deserialize_with = "null_as_empty")]
    cap_map: HashMap<String, Value>,
}

impl TailscaleNodeJson {
    fn sorted_capability_names(&self) -> Vec<String> {
        let mut capability_names: Vec<String> = self
            .cap_map
            .keys()
            .map(|capability_name| {
                capability_name
                    .strip_prefix(CAPABILITY_URL_PREFIX)
                    .unwrap_or(capability_name)
                    .to_owned()
            })
            .collect();
        capability_names.sort();
        capability_names
    }

    pub(super) fn short_name(&self) -> String {
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

impl TryFrom<&TailscaleNodeJson> for Peer {
    type Error = Error;

    fn try_from(node: &TailscaleNodeJson) -> Result<Self, Error> {
        let ip = node.tailscale_ips.first().ok_or(Error::Malformed)?;
        Ok(Peer {
            name: node.short_name(),
            ip: ip.clone(),
            os: node.os.clone(),
            path: node.peer_path(),
            received_bytes: node.rx_bytes,
            sent_bytes: node.tx_bytes,
            tags: node.tags.clone(),
        })
    }
}

impl SnapshotSource for Tailscale {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Tailscale";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("tailscale")).while_held(
            move || {
                PeriodicPolling::every(Duration::from_secs(10))
                    .poll_deliveries(move || Self::poll(()))
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

impl Tailscale {
    async fn poll(_key: ()) -> Result<Self, Error> {
        let status_json = shell_command_output::output("tailscale", &["status", "--json"]).await?;
        let status: TailscaleStatusJson =
            serde_json::from_str(&status_json).map_err(|_| Error::Malformed)?;
        match status.backend_state.as_str() {
            STARTING_BACKEND_STATE => Ok(Tailscale::Starting),
            RUNNING_BACKEND_STATE => {
                let prefs_json =
                    shell_command_output::output("tailscale", &["debug", "prefs"]).await?;
                let prefs: TailscalePrefsJson =
                    serde_json::from_str(&prefs_json).map_err(|_| Error::Malformed)?;
                Self::running_from_status(status, &prefs)
            }
            _ => Ok(Tailscale::Stopped),
        }
    }

    fn running_from_status(
        status: TailscaleStatusJson,
        prefs: &TailscalePrefsJson,
    ) -> Result<Self, Error> {
        let exit_offer = ExitNodeOffer::from_status_and_prefs(&status, prefs);
        let exit_node_in_use = status.peer.values().any(|peer| peer.carries_exit_traffic);
        let own_node = status.own_node.as_ref().ok_or(Error::Malformed)?;
        let tailnet_name = status.current_tailnet.ok_or(Error::Malformed)?.name;
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
            own_capabilities: own_node.sorted_capability_names(),
            exit_offer,
            exit_node_in_use,
            key_expiry: own_node
                .key_expiry
                .map_or(KeyExpiry::Disabled, KeyExpiry::At),
            health_warnings: status.health,
        })))
    }
}

impl Tailscale {
    #[must_use]
    pub fn switch(&self) -> TailscaleSwitch {
        match self {
            Tailscale::Stopped => TailscaleSwitch::Up,
            Tailscale::Starting | Tailscale::Running(_) => TailscaleSwitch::Down,
        }
    }
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
            "KeyExpiry": "2026-12-08T19:22:27Z",
            "Tags": ["tag:home"],
            "ExitNodeOption": true,
            "CapMap": {"https://tailscale.com/cap/ssh": null, "default-auto-update": [true]}
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

    const ADVERTISING_PREFS: &str = r#"{"AdvertiseRoutes": ["0.0.0.0/0", "::/0"]}"#;
    const PLAIN_PREFS: &str = r#"{"AdvertiseRoutes": null}"#;

    fn parsed_tailnet(status_json: &str, prefs_json: &str) -> Tailnet {
        let status: TailscaleStatusJson = serde_json::from_str(status_json).expect("status");
        let prefs: TailscalePrefsJson = serde_json::from_str(prefs_json).expect("prefs");
        match Tailscale::running_from_status(status, &prefs) {
            Ok(Tailscale::Running(tailnet)) => *tailnet,
            other => panic!("expected running tailnet, got {other:?}"),
        }
    }

    fn running_tailnet() -> Tailnet {
        parsed_tailnet(STATUS_FIXTURE, ADVERTISING_PREFS)
    }

    #[test]
    fn an_offering_machine_can_stop_offering() {
        let tailnet = running_tailnet();
        assert_eq!(tailnet.exit_offer, ExitNodeOffer::Approved);
        assert_eq!(
            tailnet.offer_control(),
            ExitOfferControl::Switchable(ExitOfferSwitch::StopOffering)
        );
    }

    #[test]
    fn using_an_exit_node_blocks_starting_to_offer() {
        let tailnet = parsed_tailnet(STATUS_FIXTURE, PLAIN_PREFS);
        assert!(tailnet.exit_node_in_use);
        assert_eq!(tailnet.exit_offer, ExitNodeOffer::NotOffered);
        assert_eq!(
            tailnet.offer_control(),
            ExitOfferControl::BlockedByExitNodeInUse
        );
    }

    #[test]
    fn an_idle_machine_can_start_offering() {
        let idle_status = STATUS_FIXTURE.replace(r#", "ExitNode": true"#, "");
        assert_eq!(
            parsed_tailnet(&idle_status, PLAIN_PREFS).offer_control(),
            ExitOfferControl::Switchable(ExitOfferSwitch::StartOffering)
        );
    }

    #[test]
    fn an_unapproved_advertisement_awaits_approval() {
        let unapproved =
            STATUS_FIXTURE.replace(r#""ExitNodeOption": true"#, r#""ExitNodeOption": false"#);
        assert_eq!(
            parsed_tailnet(&unapproved, ADVERTISING_PREFS).exit_offer,
            ExitNodeOffer::AwaitingApproval
        );
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
    fn own_access_tailnet_and_key_expiry_are_read() {
        let tailnet = running_tailnet();
        assert_eq!(tailnet.own_node.tags, ["tag:home"]);
        assert_eq!(tailnet.own_capabilities, ["default-auto-update", "ssh"]);
        assert_eq!(tailnet.tailnet_name, "example.com");
        assert!(matches!(tailnet.key_expiry, KeyExpiry::At(_)));
        assert!(tailnet.health_warnings.is_empty());
    }
}
