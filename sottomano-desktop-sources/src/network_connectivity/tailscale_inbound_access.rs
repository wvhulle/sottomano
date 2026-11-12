use std::{
    collections::{BTreeSet, HashMap},
    fmt::{self, Display, Formatter},
    net::IpAddr,
    str::FromStr,
    time::Duration,
};

use futures::{Stream, stream};
use ipnet::IpNet;
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    backing_service_availability::AvailabilityStreams,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::tailscale_status::{
        Error, TailscaleNodeJson, TailscaleStatusJson, null_as_empty,
    },
    system_protocol_clients::shell_command_output,
};

const ANY_ADDRESS: &str = "*";
const ANY_SOURCE_NAME: &str = "anyone";
const SERVICE_NAME_PREFIX: &str = "svc:";
const HIGHEST_PORT: u16 = 65535;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum InboundDestination {
    ThisMachine,
    HostedService { service_name: String },
    SubnetRoute { network: String },
    ExitNodeInternet,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum InboundPorts {
    All,
    Single(u16),
    Range { first: u16, last: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InboundAccessRule {
    pub source_name: String,
    pub destination: InboundDestination,
    pub ports: InboundPorts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleInboundAccess {
    pub rules: Vec<InboundAccessRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
enum FilterAddress {
    Any,
    Network(IpNet),
    Range { first: IpAddr, last: IpAddr },
}

#[derive(Debug)]
struct UnreadableFilterAddress {
    text: String,
}

impl Display for UnreadableFilterAddress {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "unreadable packet filter address {}", self.text)
    }
}

impl FromStr for FilterAddress {
    type Err = UnreadableFilterAddress;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let unreadable = || UnreadableFilterAddress {
            text: text.to_owned(),
        };
        if text == ANY_ADDRESS {
            return Ok(FilterAddress::Any);
        }
        if text.contains('/') {
            return text
                .parse()
                .map(FilterAddress::Network)
                .map_err(|_| unreadable());
        }
        match text.split_once('-') {
            Some((first, last)) => Ok(FilterAddress::Range {
                first: first.parse().map_err(|_| unreadable())?,
                last: last.parse().map_err(|_| unreadable())?,
            }),
            None => text
                .parse::<IpAddr>()
                .map(|host_address| FilterAddress::Network(IpNet::from(host_address)))
                .map_err(|_| unreadable()),
        }
    }
}

impl TryFrom<String> for FilterAddress {
    type Error = UnreadableFilterAddress;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl Display for FilterAddress {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match (self, self.host_address()) {
            (FilterAddress::Any, _) => formatter.write_str(ANY_SOURCE_NAME),
            (FilterAddress::Network(_), Some(host_address)) => host_address.fmt(formatter),
            (FilterAddress::Network(network), None) => network.fmt(formatter),
            (FilterAddress::Range { first, last }, _) => write!(formatter, "{first}-{last}"),
        }
    }
}

impl FilterAddress {
    fn host_address(&self) -> Option<IpAddr> {
        match self {
            FilterAddress::Network(network) if network.prefix_len() == network.max_prefix_len() => {
                Some(network.addr())
            }
            FilterAddress::Any | FilterAddress::Network(_) | FilterAddress::Range { .. } => None,
        }
    }

    fn within(&self, route: &IpNet) -> bool {
        match self {
            FilterAddress::Any => false,
            FilterAddress::Network(network) => route.contains(network),
            FilterAddress::Range { first, last } => route.contains(first) && route.contains(last),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NetmapJson {
    #[serde(default, deserialize_with = "null_as_empty")]
    packet_filter_rules: Vec<FilterRuleJson>,
    #[serde(default, deserialize_with = "null_as_empty")]
    self_node: SelfNodeJson,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SelfNodeJson {
    #[serde(default, deserialize_with = "null_as_empty")]
    addresses: Vec<IpNet>,
    #[serde(default, deserialize_with = "null_as_empty")]
    primary_routes: Vec<IpNet>,
    #[serde(default, deserialize_with = "null_as_empty")]
    cap_map: SelfCapabilitiesJson,
}

#[derive(Default, Deserialize)]
struct SelfCapabilitiesJson {
    #[serde(rename = "service-host", default, deserialize_with = "null_as_empty")]
    service_hosts: Vec<HashMap<String, Vec<IpAddr>>>,
}

#[derive(Deserialize)]
struct FilterRuleJson {
    #[serde(rename = "SrcIPs", default, deserialize_with = "null_as_empty")]
    source_addresses: Vec<FilterAddress>,
    #[serde(rename = "DstPorts", default, deserialize_with = "null_as_empty")]
    destinations: Vec<NetPortRangeJson>,
}

#[derive(Deserialize)]
struct NetPortRangeJson {
    #[serde(rename = "IP")]
    address: FilterAddress,
    #[serde(rename = "Ports")]
    ports: PortRangeJson,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PortRangeJson {
    first: u16,
    last: u16,
}

impl From<&PortRangeJson> for InboundPorts {
    fn from(ports: &PortRangeJson) -> Self {
        match (ports.first, ports.last) {
            (0, HIGHEST_PORT) => InboundPorts::All,
            (first, last) if first == last => InboundPorts::Single(first),
            (first, last) => InboundPorts::Range { first, last },
        }
    }
}

struct NodeNamesByAddress {
    names: HashMap<IpAddr, String>,
}

impl NodeNamesByAddress {
    fn from_status(status: &TailscaleStatusJson) -> Self {
        let names = status
            .own_node
            .iter()
            .chain(status.peer.values())
            .flat_map(|node: &TailscaleNodeJson| {
                node.tailscale_ips
                    .iter()
                    .map(|address| (*address, node.short_name()))
            })
            .collect();
        Self { names }
    }

    fn source_name(&self, source_address: &FilterAddress) -> String {
        source_address
            .host_address()
            .and_then(|host_address| self.names.get(&host_address).cloned())
            .unwrap_or_else(|| source_address.to_string())
    }
}

struct DestinationClassifier<'netmap> {
    own_addresses: Vec<IpAddr>,
    service_names: HashMap<IpAddr, String>,
    primary_routes: &'netmap [IpNet],
}

impl<'netmap> DestinationClassifier<'netmap> {
    fn from_self_node(self_node: &'netmap SelfNodeJson) -> Self {
        let service_names = self_node
            .cap_map
            .service_hosts
            .iter()
            .flatten()
            .flat_map(|(service_name, service_addresses)| {
                let bare_service_name = service_name
                    .strip_prefix(SERVICE_NAME_PREFIX)
                    .unwrap_or(service_name)
                    .to_owned();
                service_addresses
                    .iter()
                    .map(move |service_address| (*service_address, bare_service_name.clone()))
            })
            .collect();
        Self {
            own_addresses: self_node.addresses.iter().map(IpNet::addr).collect(),
            service_names,
            primary_routes: &self_node.primary_routes,
        }
    }

    fn is_own_address(&self, address: &FilterAddress) -> bool {
        address
            .host_address()
            .is_some_and(|host_address| self.own_addresses.contains(&host_address))
    }

    fn classify(&self, destination: &FilterAddress) -> InboundDestination {
        if *destination == FilterAddress::Any || self.is_own_address(destination) {
            return InboundDestination::ThisMachine;
        }
        if let Some(service_name) = destination
            .host_address()
            .and_then(|host_address| self.service_names.get(&host_address))
        {
            return InboundDestination::HostedService {
                service_name: service_name.clone(),
            };
        }
        self.primary_routes
            .iter()
            .find(|route| destination.within(route))
            .map_or(InboundDestination::ExitNodeInternet, |route| {
                InboundDestination::SubnetRoute {
                    network: route.to_string(),
                }
            })
    }
}

impl SnapshotSource for TailscaleInboundAccess {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Tailscale packet filter";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::program_availability("tailscale")).while_available(
            move || {
                PeriodicPolling::every(Duration::from_mins(1))
                    .poll_deliveries(move || Self::poll(()))
            },
            SnapshotDelivery::SourceUnavailable,
        )
    }
}

impl TailscaleInboundAccess {
    async fn poll(_key: ()) -> Result<Self, Error> {
        let status_json = shell_command_output::output("tailscale", &["status", "--json"]).await?;
        let netmap_json = shell_command_output::output("tailscale", &["debug", "netmap"]).await?;
        Self::from_status_and_netmap(&status_json, &netmap_json)
    }

    fn from_status_and_netmap(status_json: &str, netmap_json: &str) -> Result<Self, Error> {
        let status: TailscaleStatusJson =
            serde_json::from_str(status_json).map_err(|_| Error::Malformed)?;
        let netmap: NetmapJson = serde_json::from_str(netmap_json).map_err(|_| Error::Malformed)?;
        let node_names = NodeNamesByAddress::from_status(&status);
        let classifier = DestinationClassifier::from_self_node(&netmap.self_node);
        let unique_rules: BTreeSet<InboundAccessRule> = netmap
            .packet_filter_rules
            .iter()
            .flat_map(|rule| {
                rule.source_addresses
                    .iter()
                    .filter(|source_address| !classifier.is_own_address(source_address))
                    .flat_map(|source_address| {
                        let source_name = node_names.source_name(source_address);
                        rule.destinations
                            .iter()
                            .map(|destination| InboundAccessRule {
                                source_name: source_name.clone(),
                                destination: classifier.classify(&destination.address),
                                ports: InboundPorts::from(&destination.ports),
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        Ok(Self {
            rules: unique_rules.into_iter().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS_FIXTURE: &str = r#"{
        "BackendState": "Running",
        "Self": {"DNSName": "ryzen.tail.ts.net.", "TailscaleIPs": ["100.1.1.9", "fd7a::9"]},
        "Peer": {
            "a": {"DNSName": "zbook.tail.ts.net.", "TailscaleIPs": ["100.1.1.1", "fd7a::1"]}
        }
    }"#;
    const NETMAP_FIXTURE: &str = r#"{
        "MachineKey": "mkey:unrelated",
        "Peers": [{"unexpected": {"shape": true}}],
        "SelfNode": {
            "Addresses": ["100.1.1.9/32", "fd7a::9/128"],
            "PrimaryRoutes": ["100.99.0.1/32", "fd7a::99/128", "192.168.1.0/24"],
            "CapMap": {
                "https": null,
                "service-host": [{"svc:nextcloud": ["100.99.0.1", "fd7a::99"]}]
            }
        },
        "PacketFilterRules": [
            {"SrcIPs": ["100.1.1.1", "fd7a::1", "100.1.1.9", "fd7a::9"],
             "DstPorts": [{"IP": "*", "Ports": {"First": 0, "Last": 65535}}]},
            {"SrcIPs": ["100.1.1.1/32", "100.200.0.0/16"],
             "DstPorts": [{"IP": "100.1.1.9", "Ports": {"First": 443, "Last": 443}},
                          {"IP": "100.99.0.1", "Ports": {"First": 443, "Last": 443}},
                          {"IP": "fd7a::99", "Ports": {"First": 443, "Last": 443}},
                          {"IP": "192.168.1.20", "Ports": {"First": 22, "Last": 22}},
                          {"IP": "11.0.0.0-100.63.255.255", "Ports": {"First": 0, "Last": 65535}},
                          {"IP": "2000::-3fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", "Ports": {"First": 0, "Last": 65535}}]},
            {"SrcIPs": ["*"], "DstPorts": [{"IP": "*", "Ports": {"First": 22, "Last": 22}}]}
        ]
    }"#;

    fn parsed_rules() -> Vec<InboundAccessRule> {
        TailscaleInboundAccess::from_status_and_netmap(STATUS_FIXTURE, NETMAP_FIXTURE)
            .expect("parsed fixture")
            .rules
    }

    fn zbook_rule(destination: InboundDestination, ports: InboundPorts) -> InboundAccessRule {
        InboundAccessRule {
            source_name: "zbook".to_owned(),
            destination,
            ports,
        }
    }

    #[test]
    fn filter_addresses_parse_every_shape() {
        assert_eq!("*".parse::<FilterAddress>().ok(), Some(FilterAddress::Any));
        assert_eq!(
            "100.1.1.1"
                .parse::<FilterAddress>()
                .ok()
                .and_then(|address| address.host_address()),
            "100.1.1.1".parse().ok()
        );
        assert!(matches!(
            "100.200.0.0/16".parse(),
            Ok(FilterAddress::Network(_))
        ));
        assert!(matches!(
            "2000::-3fff::".parse(),
            Ok(FilterAddress::Range { .. })
        ));
        assert!("not-an-address".parse::<FilterAddress>().is_err());
    }

    #[test]
    fn addresses_of_one_peer_merge_under_its_name() {
        let rules = parsed_rules();
        assert!(rules.contains(&zbook_rule(
            InboundDestination::ThisMachine,
            InboundPorts::All
        )));
        assert_eq!(
            rules
                .iter()
                .filter(
                    |parsed| parsed.destination == InboundDestination::ThisMachine
                        && parsed.ports == InboundPorts::All
                )
                .count(),
            1
        );
    }

    #[test]
    fn destinations_split_by_what_they_reach() {
        let rules = parsed_rules();
        assert!(rules.contains(&zbook_rule(
            InboundDestination::ThisMachine,
            InboundPorts::Single(443)
        )));
        assert!(rules.contains(&zbook_rule(
            InboundDestination::HostedService {
                service_name: "nextcloud".to_owned()
            },
            InboundPorts::Single(443)
        )));
        assert!(rules.contains(&zbook_rule(
            InboundDestination::SubnetRoute {
                network: "192.168.1.0/24".to_owned()
            },
            InboundPorts::Single(22)
        )));
        assert!(rules.contains(&zbook_rule(
            InboundDestination::ExitNodeInternet,
            InboundPorts::All
        )));
    }

    #[test]
    fn both_service_addresses_merge_into_one_rule() {
        let service_rules = parsed_rules()
            .into_iter()
            .filter(|parsed| {
                parsed.source_name == "zbook"
                    && matches!(parsed.destination, InboundDestination::HostedService { .. })
            })
            .count();
        assert_eq!(service_rules, 1);
    }

    #[test]
    fn unknown_ranges_and_wildcards_keep_readable_sources() {
        let rules = parsed_rules();
        assert!(
            rules
                .iter()
                .any(|parsed| parsed.source_name == "100.200.0.0/16")
        );
        assert!(rules.contains(&InboundAccessRule {
            source_name: "anyone".to_owned(),
            destination: InboundDestination::ThisMachine,
            ports: InboundPorts::Single(22),
        }));
    }

    #[test]
    fn this_machine_is_not_listed_as_reaching_itself() {
        assert!(
            parsed_rules()
                .iter()
                .all(|parsed| parsed.source_name != "ryzen")
        );
    }

    #[test]
    fn null_rule_lists_read_as_empty() {
        let access = TailscaleInboundAccess::from_status_and_netmap(
            STATUS_FIXTURE,
            r#"{"SelfNode": null, "PacketFilterRules": [{"SrcIPs": null, "DstPorts": null}]}"#,
        )
        .expect("parsed");
        assert!(access.rules.is_empty());
    }

    #[test]
    fn a_netmap_without_rules_means_no_inbound_access() {
        let access =
            TailscaleInboundAccess::from_status_and_netmap(STATUS_FIXTURE, "{}").expect("parsed");
        assert!(access.rules.is_empty());
    }
}
