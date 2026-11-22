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
            "CapMap": {"https://tailscale.com/cap/ssh": null, "default-auto-update": [true], "services/jellyfin": null, "services/bazarr": null}
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
    assert_eq!(tailnet.exit_node_use, ExitNodeUse::InUse);
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
    assert_eq!(
        tailnet.own_capabilities.unscoped_features,
        ["default-auto-update", "ssh"]
    );
    assert_eq!(
        tailnet.own_capabilities.scoped_by_namespace,
        BTreeMap::from([(
            "services".to_owned(),
            vec!["bazarr".to_owned(), "jellyfin".to_owned()]
        )])
    );
    assert_eq!(tailnet.tailnet_name, "example.com");
    assert!(matches!(tailnet.key_expiry, KeyExpiry::At(_)));
    assert!(tailnet.health_warnings.is_empty());
}
