use sottomano_desktop_sources::network_connectivity::{
    tailscale_exit_nodes::{
        ExitNodeChoices, ExitNodeLanAccess, ExitNodeOption, ExitNodeReachability,
    },
    tailscale_status::{
        ExitNodeOffer, ExitNodeUse, KeyExpiry, NodeCapabilities, Peer, PeerPath, Tailnet,
    },
};

use super::*;
use crate::group_indicators::{
    indicator_gestures::IndicatorGesture, indicator_group_interaction::IndicatorGroupInteraction,
};

fn tailnet_offering(exit_offer: ExitNodeOffer, exit_node_use: ExitNodeUse) -> Tailnet {
    Tailnet {
        tailnet_name: "example.ts.net".to_owned(),
        own_node: Peer {
            name: "workstation".to_owned(),
            ip: "100.1.1.9".to_owned(),
            os: "linux".to_owned(),
            path: PeerPath::Idle,
            received_bytes: 0,
            sent_bytes: 0,
            tags: Vec::new(),
        },
        online: Vec::new(),
        total_peers: 0,
        own_capabilities: NodeCapabilities::default(),
        exit_offer,
        exit_node_use,
        key_expiry: KeyExpiry::Disabled,
        health_warnings: Vec::new(),
    }
}

#[test]
fn the_offer_indicator_is_inert_while_an_exit_node_is_in_use() {
    let tailnet = tailnet_offering(ExitNodeOffer::NotOffered, ExitNodeUse::InUse);
    assert!(matches!(
        TailnetGroup::exit_offer_indicator(&tailnet)
            .gestures
            .clone()
            .left,
        IndicatorGesture::Inert
    ));
}

fn group_running_tailnet(tailnet_group: TailnetGroup) -> TailnetGroup {
    let mut running_group = tailnet_group;
    let _ = running_group.update(
        TailnetMessage::Tailscale(SnapshotDelivery::live_push(Ok(Tailscale::Running(
            Box::new(tailnet_offering(
                ExitNodeOffer::NotOffered,
                ExitNodeUse::Unused,
            )),
        )))),
        Clock::real(),
    );
    running_group
}

fn lists_policy_file_indicator(tailnet_group: &TailnetGroup) -> bool {
    tailnet_group
        .group_content(Clock::real())
        .indicator_keys()
        .iter()
        .any(|indicator_key| indicator_key.name == "Policy file")
}

#[test]
fn the_policy_file_indicator_needs_an_api_token() {
    let without_token = group_running_tailnet(TailnetGroup::without_policy_file());
    assert!(!lists_policy_file_indicator(&without_token));
    let with_token = group_running_tailnet(TailnetGroup::with_policy_file(PathBuf::from("token")));
    assert!(lists_policy_file_indicator(&with_token));
}

#[test]
fn an_approved_offer_can_be_stopped() {
    let tailnet = tailnet_offering(ExitNodeOffer::Approved, ExitNodeUse::Unused);
    assert!(matches!(
        TailnetGroup::exit_offer_indicator(&tailnet)
            .gestures
            .clone()
            .left,
        IndicatorGesture::Performs {
            message: IndicatorGroupInteraction::Tailnet(TailnetInteraction::SwitchExitOffer(
                ExitOfferSwitch::StopOffering
            )),
            ..
        }
    ));
}

fn reachable_exit_node(reachability: ExitNodeReachability) -> ExitNodeOption {
    ExitNodeOption {
        name: "zbook".to_owned(),
        ip: "100.1.1.1".to_owned(),
        reachability,
        node_use: ExitNodeUse::Unused,
    }
}

fn exit_node_choices_while(exit_offer: ExitNodeOffer) -> ExitNodeChoices {
    ExitNodeChoices {
        options: Vec::new(),
        offer: exit_offer,
        lan_access: ExitNodeLanAccess::Blocked,
    }
}

#[test]
fn an_online_exit_node_indicator_can_be_pressed() {
    assert!(!matches!(
        TailnetGroup::exit_node_choice_indicator(
            &reachable_exit_node(ExitNodeReachability::Online),
            &exit_node_choices_while(ExitNodeOffer::NotOffered)
        )
        .gestures
        .clone()
        .left,
        IndicatorGesture::Inert
    ));
}

#[test]
fn an_exit_node_indicator_is_inert_while_this_machine_advertises() {
    assert!(matches!(
        TailnetGroup::exit_node_choice_indicator(
            &reachable_exit_node(ExitNodeReachability::Online),
            &exit_node_choices_while(ExitNodeOffer::Approved)
        )
        .gestures
        .clone()
        .left,
        IndicatorGesture::Inert
    ));
}

#[test]
fn an_offline_exit_node_indicator_is_inert() {
    assert!(matches!(
        TailnetGroup::exit_node_choice_indicator(
            &reachable_exit_node(ExitNodeReachability::OfflineNeverSeen),
            &exit_node_choices_while(ExitNodeOffer::NotOffered)
        )
        .gestures
        .clone()
        .left,
        IndicatorGesture::Inert
    ));
}
