#[expect(
    clippy::disallowed_types,
    reason = "imported for test fixtures that share state with closures under test"
)]
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};

use corredo::popovers::NestedMenuOpenTrigger;
use iced::{
    Point, Rectangle, Size,
    time::{Duration, Instant},
    window::Id,
};
use sottomano_toml_config::screen_edges::Position;

use super::{ExpansionState, HIDE_GRACE, HideSchedule, HoverCardHost, KeyboardGrab};
use crate::{
    clock::Clock,
    hover_cards::{
        HoverCardMessage, HoverTarget, PointerPresence, WidgetAnchor,
        hover_card_surfaces::HoverEffect, hover_card_transition::CardRetention,
        laid_out_anchors::WidgetLiveness,
    },
    layer_surfaces::PanelEdge,
};

#[expect(
    clippy::disallowed_types,
    reason = "tests need one widget liveness handle that outlives every anchor they lay out"
)]
static LIVE_WIDGET: LazyLock<Arc<()>> = LazyLock::new(|| Arc::new(()));

fn top_edge() -> PanelEdge {
    PanelEdge {
        output: "DP-1".to_owned(),
        position: Position::Top,
    }
}

fn workspace_card() -> HoverTarget {
    HoverTarget::Workspace(7)
}

fn leave_every_way(host: &mut HoverCardHost, now: Instant) -> Instant {
    let _exited = host.update(
        HoverCardMessage::Exit {
            target: workspace_card(),
        },
        now,
    );
    let _left_card = host.update(HoverCardMessage::ExitHoverCard, now);
    let due = host.leave_after_grace(now);
    host.leave_elapsed(due + HIDE_GRACE, None);
    due
}

#[test]
fn a_held_open_card_outlasts_the_pointer_leaving() {
    let now = Clock::real().instant;
    let mut host = HoverCardHost::default();
    host.open_until_dismissed(&top_edge(), workspace_card());
    let _due = leave_every_way(&mut host, now);
    assert_eq!(host.target(), Some(&workspace_card()));
    assert_eq!(
        host.transition.shown_retention(),
        CardRetention::UntilDismissed
    );
    assert_eq!(host.hide_schedule, HideSchedule::Unscheduled);
}

#[test]
fn a_hovered_card_closes_when_the_pointer_leaves() {
    let now = Clock::real().instant;
    let mut host = HoverCardHost::default();
    host.open(&top_edge(), workspace_card());
    assert_ne!(
        host.transition.shown_retention(),
        CardRetention::UntilDismissed
    );
    let _due = leave_every_way(&mut host, now);
    assert_eq!(host.target(), None);
}

fn anchor_at(horizontal_position: f32) -> Rectangle {
    Rectangle::new(Point::new(horizontal_position, 0.0), Size::new(10.0, 10.0))
}

fn widget_at(horizontal_position: f32, opening: NestedMenuOpenTrigger) -> WidgetAnchor {
    WidgetAnchor {
        target: workspace_card(),
        opening,
        anchor: anchor_at(horizontal_position),
        visible: true,
        presence: Arc::downgrade(&LIVE_WIDGET),
    }
}

#[test]
fn a_card_notices_when_its_widget_was_dropped() {
    let widget = Arc::new(());
    let mut host = HoverCardHost::default();
    remember(
        &mut host,
        "DP-1",
        WidgetAnchor {
            presence: Arc::downgrade(&widget),
            ..widget_at(1.0, NestedMenuOpenTrigger::Hover)
        },
    );
    host.open(&top_edge(), workspace_card());
    assert_eq!(host.shown_widget_liveness(), WidgetLiveness::Alive);
    drop(widget);
    assert_eq!(host.shown_widget_liveness(), WidgetLiveness::Dropped);
}

#[test]
fn a_widget_reported_again_after_a_rebuild_keeps_its_card() {
    let rebuilt = Arc::new(());
    let mut host = HoverCardHost::default();
    {
        let original = Arc::new(());
        remember(
            &mut host,
            "DP-1",
            WidgetAnchor {
                presence: Arc::downgrade(&original),
                ..widget_at(1.0, NestedMenuOpenTrigger::Hover)
            },
        );
        host.open(&top_edge(), workspace_card());
    }
    assert_eq!(host.shown_widget_liveness(), WidgetLiveness::Dropped);
    remember(
        &mut host,
        "DP-1",
        WidgetAnchor {
            presence: Arc::downgrade(&rebuilt),
            ..widget_at(1.0, NestedMenuOpenTrigger::Hover)
        },
    );
    assert_eq!(host.shown_widget_liveness(), WidgetLiveness::Alive);
}

#[test]
fn a_hiding_card_does_not_count_as_dropped() {
    let now = Clock::real().instant;
    let mut host = HoverCardHost::default();
    host.open(&top_edge(), workspace_card());
    host.dismiss(None, now);
    assert_eq!(host.shown_widget_liveness(), WidgetLiveness::Alive);
}

fn remember(host: &mut HoverCardHost, output: &str, widget_anchor: WidgetAnchor) {
    let _hide_due = host.remember_anchor(output, widget_anchor, Clock::real().instant);
}

#[test]
fn an_open_card_reads_the_newest_anchor_of_its_output() {
    let mut host = HoverCardHost::default();
    let opening = NestedMenuOpenTrigger::Hover;
    remember(&mut host, "DP-1", widget_at(1.0, opening));
    remember(&mut host, "DP-2", widget_at(50.0, opening));
    host.open(&top_edge(), workspace_card());
    assert_eq!(host.shown_anchor(), Some(anchor_at(1.0)));
    remember(&mut host, "DP-1", widget_at(2.0, opening));
    assert_eq!(host.shown_anchor(), Some(anchor_at(2.0)));
}

#[test]
fn a_held_open_menu_reads_the_newest_anchor_of_its_handle() {
    let mut host = host_holding_a_menu_open();
    let opening = NestedMenuOpenTrigger::Press;
    remember(&mut host, "DP-1", widget_at(1.0, opening));
    remember(&mut host, "DP-1", widget_at(2.0, opening));
    assert_eq!(host.shown_anchor(), Some(anchor_at(2.0)));
}

#[test]
fn a_card_without_a_reported_anchor_has_no_position() {
    let host = host_holding_a_menu_open();
    assert_eq!(host.shown_anchor(), None);
}

#[test]
fn a_shown_card_leaves_when_its_widget_is_hidden_even_with_the_pointer_on_it() {
    let now = Clock::real().instant;
    let mut host = HoverCardHost::default();
    remember(
        &mut host,
        "DP-1",
        widget_at(1.0, NestedMenuOpenTrigger::Hover),
    );
    host.open(&top_edge(), workspace_card());
    let _on_card = host.update(HoverCardMessage::EnterHoverCard, now);
    let due = host
        .remember_anchor(
            "DP-1",
            WidgetAnchor {
                visible: false,
                ..widget_at(1.0, NestedMenuOpenTrigger::Hover)
            },
            now,
        )
        .expect("the hidden widget's card leaves");
    host.leave_elapsed(due, None);
    assert_eq!(host.target(), None);
}

#[test]
fn a_hidden_widget_on_another_output_leaves_the_card_alone() {
    let now = Clock::real().instant;
    let mut host = HoverCardHost::default();
    remember(
        &mut host,
        "DP-1",
        widget_at(1.0, NestedMenuOpenTrigger::Hover),
    );
    host.open(&top_edge(), workspace_card());
    let hide_due = host.remember_anchor(
        "DP-2",
        WidgetAnchor {
            visible: false,
            ..widget_at(1.0, NestedMenuOpenTrigger::Hover)
        },
        now,
    );
    assert_eq!(hide_due, None);
}

#[test]
fn a_held_open_menu_stays_when_its_handle_is_hidden() {
    let now = Clock::real().instant;
    let mut host = host_holding_a_menu_open();
    let due = host.remember_anchor(
        "DP-1",
        WidgetAnchor {
            visible: false,
            ..widget_at(1.0, NestedMenuOpenTrigger::Press)
        },
        now,
    );
    host.leave_elapsed(due.map_or(now, |due| due + HIDE_GRACE), None);
    assert_eq!(host.target(), Some(&workspace_card()));
}

#[test]
fn a_disconnect_removes_anchors_and_the_shown_card() {
    let mut host = HoverCardHost::default();
    remember(
        &mut host,
        "DP-1",
        widget_at(1.0, NestedMenuOpenTrigger::Hover),
    );
    host.open(&top_edge(), workspace_card());
    host.forget_output("DP-1");
    assert_eq!(host.target(), None);
    host.open(&top_edge(), workspace_card());
    assert_eq!(host.shown_anchor(), None);
}

fn host_holding_a_menu_open() -> HoverCardHost {
    let mut host = HoverCardHost::default();
    host.open_until_dismissed(&top_edge(), workspace_card());
    host
}

fn other_menu_handle() -> HoverTarget {
    HoverTarget::Workspace(8)
}

#[test]
fn with_no_menu_open_hovering_a_handle_opens_nothing() {
    let host = HoverCardHost::default();
    assert_eq!(
        host.hover_effect(&workspace_card(), "DP-1"),
        HoverEffect::KeepsMenu
    );
}

#[test]
fn an_open_menu_switches_on_hovering_another_handle_on_its_output_only() {
    let host = host_holding_a_menu_open();
    assert_eq!(
        host.hover_effect(&other_menu_handle(), "DP-1"),
        HoverEffect::SwitchesMenu
    );
    assert_eq!(
        host.hover_effect(&workspace_card(), "DP-1"),
        HoverEffect::KeepsMenu
    );
    assert_eq!(
        host.hover_effect(&other_menu_handle(), "DP-2"),
        HoverEffect::KeepsMenu
    );
}

#[test]
fn a_handle_indicator_shows_open_only_on_the_output_of_the_card() {
    let host = host_holding_a_menu_open();
    assert_eq!(
        host.handle_indicator(&workspace_card(), "DP-1")
            .menu_expansion,
        ExpansionState::Expanded
    );
    assert_eq!(
        host.handle_indicator(&workspace_card(), "DP-2")
            .menu_expansion,
        ExpansionState::Collapsed
    );
    assert_eq!(
        host.handle_indicator(&other_menu_handle(), "DP-1")
            .menu_expansion,
        ExpansionState::Collapsed
    );
}

#[test]
fn a_handle_indicator_follows_the_pointer_and_survives_a_new_layout() {
    let mut host = HoverCardHost::default();
    remember(
        &mut host,
        "DP-1",
        widget_at(0.0, NestedMenuOpenTrigger::Press),
    );
    host.set_handle_pointing("DP-1", &workspace_card(), PointerPresence::PointedAt);
    assert_eq!(
        host.handle_indicator(&workspace_card(), "DP-1").pointing,
        PointerPresence::PointedAt
    );
    assert_eq!(
        host.handle_indicator(&workspace_card(), "DP-2").pointing,
        PointerPresence::NotPointedAt
    );
    remember(
        &mut host,
        "DP-1",
        widget_at(5.0, NestedMenuOpenTrigger::Press),
    );
    assert_eq!(
        host.handle_indicator(&workspace_card(), "DP-1").pointing,
        PointerPresence::PointedAt
    );
    host.set_handle_pointing("DP-1", &workspace_card(), PointerPresence::NotPointedAt);
    assert_eq!(
        host.handle_indicator(&workspace_card(), "DP-1").pointing,
        PointerPresence::NotPointedAt
    );
}

fn panels_on(outputs: &[&str]) -> BTreeMap<String, Id> {
    outputs
        .iter()
        .map(|output| ((*output).to_owned(), Id::unique()))
        .collect()
}

#[test]
fn a_card_closes_when_its_output_loses_the_panel() {
    let now = Clock::real().instant;
    let fade = Duration::from_millis(1);
    let mut host = host_holding_a_menu_open();
    let _kept = host.reconcile_with_panels(&panels_on(&["DP-1", "DP-2"]), now, fade);
    assert_eq!(host.target(), Some(&workspace_card()));
    let _lost = host.reconcile_with_panels(&panels_on(&["DP-2"]), now, fade);
    assert_eq!(host.target(), None);
}

#[test]
fn a_held_open_menu_grabs_the_keyboard_on_its_panel_surface() {
    let now = Clock::real().instant;
    let fade = Duration::from_millis(1);
    let panels = panels_on(&["DP-1"]);
    let mut host = host_holding_a_menu_open();
    let _grabbed = host.reconcile_with_panels(&panels, now, fade);
    assert_eq!(
        host.keyboard_grab,
        panels
            .get("DP-1")
            .copied()
            .map_or(KeyboardGrab::Released, KeyboardGrab::Held)
    );
    host.dismiss(None, now);
    let _released = host.reconcile_with_panels(&panels, now, fade);
    assert_eq!(host.keyboard_grab, KeyboardGrab::Released);
}

#[test]
fn dismissing_a_held_open_card_closes_it() {
    let now = Clock::real().instant;
    let mut host = HoverCardHost::default();
    host.open_until_dismissed(&top_edge(), workspace_card());
    host.dismiss(None, now);
    assert_eq!(host.target(), None);
    assert_ne!(
        host.transition.shown_retention(),
        CardRetention::UntilDismissed
    );
}
