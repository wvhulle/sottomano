use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use futures::{Stream, stream};
use serde::Deserialize;

use crate::{
    SnapshotDelivery, SnapshotSource,
    delivery_cadence_drivers::PeriodicPolling,
    network_connectivity::tailscale_status::{
        Error, TailscaleNodeJson, TailscaleStatusJson, null_as_empty,
    },
    stream_operators::StreamOperators,
    system_protocol_clients::shell_command_output,
};

const ANY_ADDRESS: &str = "*";
const ANY_SOURCE_NAME: &str = "anyone";
const SINGLE_HOST_SUFFIXES: [&str; 2] = ["/32", "/128"];
const HIGHEST_PORT: u16 = 65535;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InboundReach {
    ThisMachine,
    RoutesThroughThisMachine,
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
    pub reach: InboundReach,
    pub ports: InboundPorts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscaleInboundAccess {
    pub rules: Vec<InboundAccessRule>,
}

#[derive(Deserialize)]
struct NetmapPacketFilterJson {
    #[serde(
        rename = "PacketFilterRules",
        default,
        deserialize_with = "null_as_empty"
    )]
    packet_filter_rules: Vec<PacketFilterRuleJson>,
}

#[derive(Deserialize)]
struct PacketFilterRuleJson {
    #[serde(rename = "SrcIPs", default, deserialize_with = "null_as_empty")]
    source_addresses: Vec<String>,
    #[serde(rename = "DstPorts", default, deserialize_with = "null_as_empty")]
    destinations: Vec<PacketFilterDestinationJson>,
}

#[derive(Deserialize)]
struct PacketFilterDestinationJson {
    #[serde(rename = "IP")]
    address: String,
    #[serde(rename = "Ports")]
    ports: PacketFilterPortsJson,
}

#[derive(Deserialize)]
struct PacketFilterPortsJson {
    #[serde(rename = "First")]
    first: u16,
    #[serde(rename = "Last")]
    last: u16,
}

impl From<&PacketFilterPortsJson> for InboundPorts {
    fn from(ports: &PacketFilterPortsJson) -> Self {
        match (ports.first, ports.last) {
            (0, HIGHEST_PORT) => InboundPorts::All,
            (first, last) if first == last => InboundPorts::Single(first),
            (first, last) => InboundPorts::Range { first, last },
        }
    }
}

struct NodeNamesByAddress {
    names: HashMap<String, String>,
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
                    .map(|address| (address.clone(), node.short_name()))
            })
            .collect();
        Self { names }
    }

    fn source_name(&self, source_address: &str) -> String {
        if source_address == ANY_ADDRESS {
            return ANY_SOURCE_NAME.to_owned();
        }
        self.names
            .get(single_host_address(source_address))
            .cloned()
            .unwrap_or_else(|| source_address.to_owned())
    }
}

fn single_host_address(source_address: &str) -> &str {
    SINGLE_HOST_SUFFIXES
        .iter()
        .find_map(|suffix| source_address.strip_suffix(suffix))
        .unwrap_or(source_address)
}

fn reach_of_destination(destination_address: &str, own_addresses: &[String]) -> InboundReach {
    if destination_address == ANY_ADDRESS
        || own_addresses
            .iter()
            .any(|own_address| own_address == destination_address)
    {
        InboundReach::ThisMachine
    } else {
        InboundReach::RoutesThroughThisMachine
    }
}

impl SnapshotSource for TailscaleInboundAccess {
    type Error = Error;
    type StreamInputs = ();

    const NAME: &'static str = "Tailscale packet filter";

    fn delivery_stream((): ()) -> impl Stream<Item = SnapshotDelivery<Self>> + Send {
        stream::once(shell_command_output::exists("tailscale")).available_while(move || {
            PeriodicPolling::every(Duration::from_mins(1)).poll_deliveries(move || Self::poll(()))
        })
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
        let netmap: NetmapPacketFilterJson =
            serde_json::from_str(netmap_json).map_err(|_| Error::Malformed)?;
        let node_names = NodeNamesByAddress::from_status(&status);
        let own_addresses: Vec<String> = status
            .own_node
            .iter()
            .flat_map(|own_node| own_node.tailscale_ips.iter().cloned())
            .collect();
        let unique_rules: BTreeSet<InboundAccessRule> = netmap
            .packet_filter_rules
            .iter()
            .flat_map(|rule| {
                let destinations = &rule.destinations;
                rule.source_addresses
                    .iter()
                    .filter(|source_address| {
                        !own_addresses
                            .iter()
                            .any(|own_address| own_address == single_host_address(source_address))
                    })
                    .flat_map(|source_address| {
                        let source_name = node_names.source_name(source_address);
                        let own_addresses = &own_addresses;
                        destinations
                            .iter()
                            .map(move |destination| InboundAccessRule {
                                source_name: source_name.clone(),
                                reach: reach_of_destination(&destination.address, own_addresses),
                                ports: InboundPorts::from(&destination.ports),
                            })
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
        "NodeKey": "nodekey:unrelated",
        "Peers": [{"unexpected": {"shape": true}}],
        "PacketFilterRules": [
            {"SrcIPs": ["100.1.1.1", "fd7a::1", "100.1.1.9", "fd7a::9"],
             "DstPorts": [{"IP": "*", "Ports": {"First": 0, "Last": 65535}}]},
            {"SrcIPs": ["100.1.1.1/32", "100.200.0.0/16"],
             "DstPorts": [{"IP": "100.1.1.9", "Ports": {"First": 443, "Last": 443}},
                          {"IP": "11.0.0.0-100.63.255.255", "Ports": {"First": 80, "Last": 90}}]},
            {"SrcIPs": ["*"], "DstPorts": [{"IP": "*", "Ports": {"First": 22, "Last": 22}}]}
        ]
    }"#;

    fn parsed_rules() -> Vec<InboundAccessRule> {
        TailscaleInboundAccess::from_status_and_netmap(STATUS_FIXTURE, NETMAP_FIXTURE)
            .expect("parsed fixture")
            .rules
    }

    fn rule(source_name: &str, reach: InboundReach, ports: InboundPorts) -> InboundAccessRule {
        InboundAccessRule {
            source_name: source_name.to_owned(),
            reach,
            ports,
        }
    }

    #[test]
    fn addresses_of_one_peer_merge_under_its_name() {
        let rules = parsed_rules();
        let all_port_rules = rules
            .iter()
            .filter(|parsed| parsed.ports == InboundPorts::All)
            .count();
        assert_eq!(all_port_rules, 1);
        assert!(rules.contains(&rule("zbook", InboundReach::ThisMachine, InboundPorts::All)));
    }

    #[test]
    fn destinations_split_into_this_machine_and_forwarded_routes() {
        let rules = parsed_rules();
        assert!(rules.contains(&rule(
            "zbook",
            InboundReach::ThisMachine,
            InboundPorts::Single(443)
        )));
        assert!(rules.contains(&rule(
            "zbook",
            InboundReach::RoutesThroughThisMachine,
            InboundPorts::Range {
                first: 80,
                last: 90
            }
        )));
    }

    #[test]
    fn unknown_ranges_and_wildcards_keep_readable_sources() {
        let rules = parsed_rules();
        assert!(rules.contains(&rule(
            "100.200.0.0/16",
            InboundReach::ThisMachine,
            InboundPorts::Single(443)
        )));
        assert!(rules.contains(&rule(
            "anyone",
            InboundReach::ThisMachine,
            InboundPorts::Single(22)
        )));
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
            r#"{"PacketFilterRules": [{"SrcIPs": null, "DstPorts": null}]}"#,
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
