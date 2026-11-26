use std::time::Duration;

use corredo::transitions::ExpansionState;

use super::{
    AlertPromotion, GroupMenuMessage, GroupMenuPin, GroupMenuState, GroupMenuTopic,
    GroupSourceDemand, IndicatorGroupFamily, IndicatorGroupIdentity, MENU_SOURCE_LINGER,
    NestedMenuAction, OpenGroupMenu, PanelPin, PanelPinDuration, PanelRaise,
};
use crate::{
    clock::Clock,
    group_indicators::indicator_group_identity::IndicatorGroupInstance,
    hover_cards::{IndicatorKey, PanelPinOrigin},
};

fn quiet_pin(
    group_menus: &GroupMenuState,
    menu: GroupMenuTopic,
    entry: GroupMenuPin,
) -> PanelPinOrigin {
    group_menus.panel_pin(menu, entry, AlertPromotion::Unpromoted)
}

fn pinned_for(
    menu: GroupMenuTopic,
    entry: GroupMenuPin,
    duration: PanelPinDuration,
) -> PanelPinOrigin {
    PanelPinOrigin::Pinned(Box::new(PanelPin {
        menu,
        entry,
        duration,
    }))
}

const GPU_GROUP: IndicatorGroupIdentity = IndicatorGroupIdentity::sole(IndicatorGroupFamily::Gpu);

fn wired_gpu_load_indicator() -> GroupMenuPin {
    GroupMenuPin::Item(IndicatorKey {
        group: GPU_GROUP,
        name: "gpu_load".into(),
    })
}

fn toggle_in_hardware(entry: GroupMenuPin) -> GroupMenuMessage {
    GroupMenuMessage::TogglePin {
        menu: GroupMenuTopic::Hardware,
        entry,
    }
}

fn hardware_menu_open() -> OpenGroupMenu {
    OpenGroupMenu {
        topic: GroupMenuTopic::Hardware,
        families: vec![IndicatorGroupFamily::Gpu],
    }
}

fn gpu_demand(group_menus: &GroupMenuState) -> GroupSourceDemand {
    group_menus.source_demand(IndicatorGroupFamily::Gpu)
}

fn collapsed_on_panel() -> GroupSourceDemand {
    GroupSourceDemand::on_panel()
}

fn player_on_bus(player: &'static str, bus: &'static str) -> IndicatorGroupIdentity {
    IndicatorGroupIdentity {
        family: IndicatorGroupFamily::Media,
        instance: IndicatorGroupInstance::Session {
            durable: player.into(),
            session: bus.into(),
        },
    }
}

#[test]
fn toggling_twice_unpins_the_group() {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(GPU_GROUP)
        ),
        pinned_for(
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(GPU_GROUP),
            PanelPinDuration::Permanent
        )
    );
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Behaviour,
            GroupMenuPin::Group(GPU_GROUP)
        ),
        PanelPinOrigin::Unpinned
    );
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(GPU_GROUP)
        ),
        PanelPinOrigin::Unpinned
    );
}

#[test]
fn pinning_an_indicator_leaves_its_group_unpinned() {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(wired_gpu_load_indicator()));
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            wired_gpu_load_indicator()
        ),
        pinned_for(
            GroupMenuTopic::Hardware,
            wired_gpu_load_indicator(),
            PanelPinDuration::Permanent
        )
    );
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(GPU_GROUP)
        ),
        PanelPinOrigin::Unpinned
    );
}

#[test]
fn unpinning_an_expanded_group_drops_its_demand() {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
    assert_eq!(gpu_demand(&group_menus), collapsed_on_panel());
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(GPU_GROUP)));
    assert_ne!(
        gpu_demand(&group_menus).expanded_when(ExpansionState::Expanded),
        collapsed_on_panel().expanded_when(ExpansionState::Expanded)
    );
}

fn alerting_gpu_indicator() -> PanelRaise {
    PanelRaise {
        menu: GroupMenuTopic::Hardware,
        entry: alerting_gpu_pin(),
    }
}

fn alerting_gpu_pin() -> GroupMenuPin {
    GroupMenuPin::Item(IndicatorKey {
        group: GPU_GROUP,
        name: "hot gpu".into(),
    })
}

#[test]
fn a_dismissed_alert_pin_stays_dismissed_while_its_indicator_keeps_alerting() {
    let mut group_menus = GroupMenuState::default();
    group_menus.track_source_demand(None, vec![alerting_gpu_indicator()], Clock::real().instant);
    let _ = group_menus.update(GroupMenuMessage::DismissAlertPin {
        menu: GroupMenuTopic::Hardware,
        entry: alerting_gpu_pin(),
    });
    group_menus.track_source_demand(None, vec![alerting_gpu_indicator()], Clock::real().instant);
    assert_eq!(
        group_menus.panel_pin(
            GroupMenuTopic::Hardware,
            alerting_gpu_pin(),
            AlertPromotion::Promoted
        ),
        PanelPinOrigin::Unpinned
    );
}

#[test]
fn a_dismissed_alert_pin_rises_again_after_its_alert_ends() {
    let mut group_menus = GroupMenuState::default();
    group_menus.track_source_demand(None, vec![alerting_gpu_indicator()], Clock::real().instant);
    let _ = group_menus.update(GroupMenuMessage::DismissAlertPin {
        menu: GroupMenuTopic::Hardware,
        entry: alerting_gpu_pin(),
    });
    group_menus.track_source_demand(None, Vec::new(), Clock::real().instant);
    group_menus.track_source_demand(None, vec![alerting_gpu_indicator()], Clock::real().instant);
    assert_eq!(
        group_menus.panel_pin(
            GroupMenuTopic::Hardware,
            alerting_gpu_pin(),
            AlertPromotion::Promoted
        ),
        pinned_for(
            GroupMenuTopic::Hardware,
            alerting_gpu_pin(),
            PanelPinDuration::UntilAlertEnds
        )
    );
}

#[test]
fn pinning_one_sibling_indicator_leaves_the_other() {
    let sibling_indicator = |indicator_name: &str| {
        GroupMenuPin::Item(IndicatorKey {
            group: GPU_GROUP,
            name: indicator_name.to_owned().into(),
        })
    };
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(sibling_indicator(
        "WireguardTunnel(WireguardTunnelTarget { interface: \"wg0\" })",
    )));
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            sibling_indicator("WireguardTunnel(WireguardTunnelTarget { interface: \"wg0\" })")
        ),
        pinned_for(
            GroupMenuTopic::Hardware,
            sibling_indicator("WireguardTunnel(WireguardTunnelTarget { interface: \"wg0\" })"),
            PanelPinDuration::Permanent
        )
    );
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            sibling_indicator("WireguardTunnel(WireguardTunnelTarget { interface: \"wg1\" })")
        ),
        PanelPinOrigin::Unpinned
    );
}

#[test]
fn pinned_indicator_demand_is_limited_to_its_own_family() {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(wired_gpu_load_indicator()));
    assert_ne!(
        gpu_demand(&group_menus),
        group_menus.source_demand(IndicatorGroupFamily::Compute)
    );
}

#[test]
fn a_pinned_instance_demands_the_source_its_family_shares() {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(
        IndicatorGroupIdentity::durable(IndicatorGroupFamily::EthernetPorts, "enp3s0"),
    )));
    assert_eq!(
        group_menus.source_demand(IndicatorGroupFamily::EthernetPorts),
        collapsed_on_panel()
    );
    assert_ne!(
        group_menus.source_demand(IndicatorGroupFamily::WifiAdapters),
        collapsed_on_panel()
    );
}

#[test]
fn demand_outlasts_the_open_menu_for_the_linger_only() {
    let mut group_menus = GroupMenuState::default();
    let opened_at = Clock::real().instant;
    group_menus.track_source_demand(Some(hardware_menu_open()), Vec::new(), opened_at);
    assert_ne!(gpu_demand(&group_menus), collapsed_on_panel());
    group_menus.track_source_demand(None, Vec::new(), opened_at + Duration::from_secs(1));
    assert_ne!(
        gpu_demand(&group_menus),
        GroupMenuState::default().source_demand(IndicatorGroupFamily::Gpu)
    );
    group_menus.track_source_demand(None, Vec::new(), opened_at + MENU_SOURCE_LINGER);
    assert_eq!(
        gpu_demand(&group_menus),
        GroupMenuState::default().source_demand(IndicatorGroupFamily::Gpu)
    );
}

#[test]
fn a_pin_follows_its_source_not_its_position() {
    let first_port = IndicatorGroupIdentity::durable(IndicatorGroupFamily::EthernetPorts, "enp3s0");
    let second_port =
        IndicatorGroupIdentity::durable(IndicatorGroupFamily::EthernetPorts, "enp4s0");
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(second_port.clone())));
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(second_port.clone())
        ),
        pinned_for(
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(second_port),
            PanelPinDuration::Permanent
        )
    );
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(first_port)
        ),
        PanelPinOrigin::Unpinned
    );
}

#[test]
fn namesake_players_share_a_pin() {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(player_on_bus(
        "Firefox", ":1.40",
    ))));
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(player_on_bus("Firefox", ":1.41"))
        ),
        pinned_for(
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(player_on_bus("Firefox", ":1.41")),
            PanelPinDuration::Permanent
        )
    );
    assert_eq!(
        quiet_pin(
            &group_menus,
            GroupMenuTopic::Hardware,
            GroupMenuPin::Group(player_on_bus("Spotify", ":1.42"))
        ),
        PanelPinOrigin::Unpinned
    );
}

#[test]
fn closing_forgets_the_open_group() {
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(GroupMenuMessage::Menu {
        menu: GroupMenuTopic::Hardware,
        action: NestedMenuAction::Opened(vec![GPU_GROUP]),
    });
    assert_eq!(
        group_menus.inspection.open_groups(GroupMenuTopic::Hardware),
        [GPU_GROUP]
    );
    let _ = group_menus.update(GroupMenuMessage::Closed);
    assert!(
        group_menus
            .inspection
            .open_groups(GroupMenuTopic::Hardware)
            .is_empty()
    );
}

#[test]
fn an_open_group_survives_pinning_an_earlier_group() {
    let compute = IndicatorGroupIdentity::sole(IndicatorGroupFamily::Compute);
    let mut group_menus = GroupMenuState::default();
    let _ = group_menus.update(GroupMenuMessage::Menu {
        menu: GroupMenuTopic::Hardware,
        action: NestedMenuAction::Opened(vec![GPU_GROUP]),
    });
    let _ = group_menus.update(toggle_in_hardware(GroupMenuPin::Group(compute)));
    assert_eq!(
        group_menus.inspection.open_groups(GroupMenuTopic::Hardware),
        [GPU_GROUP]
    );
}
