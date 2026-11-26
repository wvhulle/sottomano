use sottomano_toml_config::{Config, screen_edges::Position, severity::Severity};

use super::{
    IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
    IndicatorListing,
};
use crate::{
    clock::Clock,
    glyph_icons::icon_glyph::IconGlyph,
    group_indicators::{group_indicator::GroupIndicator, indicator_state::IndicatorState},
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic, PanelRaise},
    hover_cards::{HoverTarget, IndicatorKey, MouseGesture, PressAction},
    indicator_groups::{
        clock_group::ClockInteraction,
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
};

fn shown_state_indicator<'indicator>(shown_state: IndicatorState) -> GroupIndicator<'indicator> {
    GroupIndicator::glyph_only("Shown state reading", IconGlyph::default(), shown_state)
        .with_hover_card(Box::new(|_| Vec::new()))
}

fn group_showing(
    expansion: &IndicatorGroupExpansion,
    shown_state: IndicatorState,
) -> IndicatorGroupContent<'_> {
    IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Weather,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::Weather.title(),
            IconGlyph::default(),
            IndicatorState::Idle,
        ),
        expansion,
        ClockInteraction::Expansion,
        IndicatorListing::Listed(Vec::from([shown_state_indicator(shown_state)])),
    )
}

#[test]
fn a_critical_indicator_rises_onto_the_panel_without_a_toast() {
    let expansion = IndicatorGroupExpansion::default();
    let group = group_showing(&expansion, IndicatorState::Critical);
    assert_eq!(
        group.panel_raises(GroupMenuTopic::Media, Severity::Critical),
        vec![PanelRaise {
            menu: GroupMenuTopic::Media,
            entry: GroupMenuPin::Item(IndicatorKey {
                group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Weather),
                name: "Shown state reading".into(),
            }),
        }]
    );
}

#[test]
fn a_warning_indicator_stays_in_its_menu_below_a_critical_threshold() {
    let expansion = IndicatorGroupExpansion::default();
    let group = group_showing(&expansion, IndicatorState::Warning);
    assert_eq!(
        group.panel_raises(GroupMenuTopic::Media, Severity::Critical),
        Vec::new()
    );
    assert_eq!(group.highest_alert_severity(), Severity::Warning);
}

#[test]
fn an_indicator_card_lists_the_current_press() {
    let expansion = IndicatorGroupExpansion::default();
    let group_pressing = |description| {
        IndicatorGroupContent::expandable(
            IndicatorGroupFamily::NetworkManagerVpns,
            GroupIndicator::glyph_only(
                IndicatorGroupFamily::NetworkManagerVpns.title(),
                IconGlyph::default(),
                IndicatorState::Idle,
            ),
            &expansion,
            ClockInteraction::Expansion,
            IndicatorListing::Listed(Vec::from([shown_state_indicator(IndicatorState::Idle)
                .on_press(
                    description,
                    ClockInteraction::Expansion(IndicatorGroupExpansionMessage::Toggle),
                )])),
        )
    };
    let indicator_key = IndicatorKey {
        group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::NetworkManagerVpns),
        name: "Shown state reading".into(),
    };
    let listed_actions = |description| {
        let group = group_pressing(description);
        assert!(
            group
                .hover_card(
                    &HoverTarget::Indicator(indicator_key.clone()),
                    Clock::real()
                )
                .is_some()
        );
        group
            .indicator(&indicator_key)
            .map(GroupIndicator::press_actions)
    };
    assert_eq!(
        listed_actions("connect the VPN"),
        Some(vec![PressAction::new(
            MouseGesture::LeftClick,
            "connect the VPN"
        )])
    );
    assert_eq!(
        listed_actions("disconnect the VPN"),
        Some(vec![PressAction::new(
            MouseGesture::LeftClick,
            "disconnect the VPN"
        )])
    );
}

#[test]
fn an_empty_group_keeps_its_panel_handle() {
    let expansion = IndicatorGroupExpansion::default();
    let config = Config::default();
    let empty_group = IndicatorGroupContent::expandable(
        IndicatorGroupFamily::Weather,
        GroupIndicator::glyph_only(
            IndicatorGroupFamily::Weather.title(),
            IconGlyph::default(),
            IndicatorState::Idle,
        ),
        &expansion,
        ClockInteraction::Expansion,
        IndicatorListing::Listed(Vec::new()),
    );
    assert_eq!(
        empty_group
            .hover_card(
                &HoverTarget::GroupHeadline(Box::new(IndicatorGroupIdentity::sole(
                    IndicatorGroupFamily::Weather
                ))),
                Clock::real(),
            )
            .map(|hover_card| hover_card.sections.len()),
        Some(0)
    );
    let _panel_handle = empty_group.view_events(&config, Position::Top, Clock::real());
}
