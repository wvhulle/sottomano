#[cfg(test)]
mod tailscale_status_tests;

use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
    time::Duration,
};

use chrono::{DateTime, Datelike, Utc};
use futures::{Stream, stream};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::{
    SnapshotDelivery, SnapshotSource, backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::PeriodicPolling, system_protocol_clients::shell_command_output,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeCapabilities {
    pub unscoped_features: Vec<String>,
    pub scoped_by_namespace: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tailnet {
    pub tailnet_name: String,
    pub own_node: Peer,
    pub online: Vec<Peer>,
    pub total_peers: usize,
    pub own_capabilities: NodeCapabilities,
    pub exit_offer: ExitNodeOffer,
    pub exit_node_use: ExitNodeUse,
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
pub enum ExitNodeUse {
    InUse,
    Unused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitNodeOffer {
    NotOffered,
    AwaitingApproval,
    Approved,
}

impl ExitNodeOffer {
    pub(super) fn reported_exit_offer(
        status: &TailscaleStatusJson,
        prefs: &TailscalePrefsJson,
    ) -> Self {
        let advertised = prefs
            .advertised_routes
            .iter()
            .any(|route| INTERNET_ROUTES.contains(&route.as_str()));
        let approved = status
            .own_node
            .as_ref()
            .is_some_and(|own_node| own_node.offers_exit_node);
        match (advertised, approved) {
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
        match (self.exit_offer, self.exit_node_use) {
            (ExitNodeOffer::AwaitingApproval | ExitNodeOffer::Approved, _) => {
                ExitOfferControl::Switchable(ExitOfferSwitch::StopOffering)
            }
            (ExitNodeOffer::NotOffered, ExitNodeUse::InUse) => {
                ExitOfferControl::BlockedByExitNodeInUse
            }
            (ExitNodeOffer::NotOffered, ExitNodeUse::Unused) => {
                ExitOfferControl::Switchable(ExitOfferSwitch::StartOffering)
            }
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
    pub(super) tailscale_ips: Vec<IpAddr>,
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
    fn grouped_capabilities(&self) -> NodeCapabilities {
        let mut capabilities = NodeCapabilities::default();
        for capability_name in self.cap_map.keys() {
            let short_name = capability_name
                .strip_prefix(CAPABILITY_URL_PREFIX)
                .unwrap_or(capability_name);
            match short_name.split_once('/') {
                Some((namespace, scoped_name)) => capabilities
                    .scoped_by_namespace
                    .entry(namespace.to_owned())
                    .or_default()
                    .push(scoped_name.to_owned()),
                None => capabilities.unscoped_features.push(short_name.to_owned()),
            }
        }
        capabilities.unscoped_features.sort();
        capabilities
            .scoped_by_namespace
            .values_mut()
            .for_each(|scoped_names| scoped_names.sort());
        capabilities
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
            ip: ip.to_string(),
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

    const NAME: &'static str = "Tailnet connection";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::program_availability("tailscale")).while_available(
            move || {
                PeriodicPolling::every(Duration::from_secs(10))
                    .poll_deliveries(move || Self::poll(()))
            },
            || SnapshotDelivery::SourceUnavailable,
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
        let exit_offer = ExitNodeOffer::reported_exit_offer(&status, prefs);
        let exit_node_use =
            status
                .peer
                .values()
                .fold(ExitNodeUse::Unused, |exit_node_use, peer| {
                    if peer.carries_exit_traffic {
                        ExitNodeUse::InUse
                    } else {
                        exit_node_use
                    }
                });
        let own_node = status.own_node.as_ref().ok_or(Error::Malformed)?;
        let tailnet_name = status.current_tailnet.ok_or(Error::Malformed)?.name;
        let mut online: Vec<Peer> = status
            .peer
            .values()
            .filter(|peer| peer.online)
            .filter_map(|peer| {
                Peer::try_from(peer)
                    .inspect_err(|malformed_peer| {
                        tracing::warn!(
                            "tailscale: skipping online peer {}: {malformed_peer}",
                            peer.short_name()
                        );
                    })
                    .ok()
            })
            .collect();
        online.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Tailscale::Running(Box::new(Tailnet {
            tailnet_name,
            own_node: Peer::try_from(own_node)?,
            online,
            total_peers: status.peer.len(),
            own_capabilities: own_node.grouped_capabilities(),
            exit_offer,
            exit_node_use,
            key_expiry: own_node
                .key_expiry
                .map_or(KeyExpiry::Disabled, KeyExpiry::At),
            health_warnings: status.health,
        })))
    }

    #[must_use]
    pub fn switch(&self) -> TailscaleSwitch {
        match self {
            Tailscale::Stopped => TailscaleSwitch::Up,
            Tailscale::Starting | Tailscale::Running(_) => TailscaleSwitch::Down,
        }
    }
}
