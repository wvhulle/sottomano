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
    TailscaleInboundAccess::parsed_status_netmap(STATUS_FIXTURE, NETMAP_FIXTURE)
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
    let access = TailscaleInboundAccess::parsed_status_netmap(
        STATUS_FIXTURE,
        r#"{"SelfNode": null, "PacketFilterRules": [{"SrcIPs": null, "DstPorts": null}]}"#,
    )
    .expect("parsed");
    assert!(access.rules.is_empty());
}

#[test]
fn a_netmap_without_rules_means_no_inbound_access() {
    let access =
        TailscaleInboundAccess::parsed_status_netmap(STATUS_FIXTURE, "{}").expect("parsed");
    assert!(access.rules.is_empty());
}
