use corredo::popovers::NestedMenuOpenTrigger;
use iced::{Point, Rectangle, Size, widget::Space};

use super::{super::HoverArea, HoverState};
use crate::hover_cards::{
    AnchorEvent, HoverCardMessage, HoverTarget, PointerPresence, WidgetAnchor,
    laid_out_anchors::{AnchorVisibility, WidgetLiveness},
};

fn hover_area_with(
    target: HoverTarget,
    opening: NestedMenuOpenTrigger,
) -> HoverArea<'static, HoverCardMessage> {
    HoverArea {
        content: Space::new().into(),
        target,
        opening,
        wrap: Box::new(|message| message),
    }
}

fn area_opening_on(opening: NestedMenuOpenTrigger) -> HoverArea<'static, HoverCardMessage> {
    hover_area_with(HoverTarget::Workspace(7), opening)
}

fn fresh_state() -> HoverState {
    HoverState::from(HoverTarget::Workspace(7))
}

fn anchor_at(horizontal_position: f32) -> Rectangle {
    Rectangle::new(Point::new(horizontal_position, 0.0), Size::new(40.0, 40.0))
}

fn whole_panel() -> Rectangle {
    Rectangle::new(Point::new(0.0, 0.0), Size::new(1000.0, 40.0))
}

fn laid_out_report(
    area: &HoverArea<'_, HoverCardMessage>,
    state: &mut HoverState,
    pointer_presence: PointerPresence,
    anchor: Rectangle,
) -> Option<HoverCardMessage> {
    area.laid_out_anchor(state, pointer_presence, anchor, &whole_panel())
}

#[test]
fn a_hover_area_stays_silent_until_first_hovered() {
    let area = area_opening_on(NestedMenuOpenTrigger::Hover);
    let mut state = fresh_state();
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::NotPointedAt,
            anchor_at(0.0)
        )
        .is_none()
    );
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::NotPointedAt,
            anchor_at(5.0)
        )
        .is_none()
    );
}

#[test]
fn a_menu_handle_reports_before_any_hover() {
    let area = area_opening_on(NestedMenuOpenTrigger::Press);
    let mut state = fresh_state();
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::NotPointedAt,
            anchor_at(0.0)
        )
        .is_some()
    );
}

#[test]
fn a_hovered_area_reports_each_move_once_even_after_the_pointer_left() {
    let area = area_opening_on(NestedMenuOpenTrigger::Hover);
    let mut state = fresh_state();
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(0.0)
        )
        .is_some()
    );
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(0.0)
        )
        .is_none()
    );
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(5.0)
        )
        .is_some()
    );
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::NotPointedAt,
            anchor_at(5.0)
        )
        .is_none()
    );
    assert!(
        laid_out_report(
            &area,
            &mut state,
            PointerPresence::NotPointedAt,
            anchor_at(9.0)
        )
        .is_some()
    );
}

#[test]
fn a_new_target_in_a_reused_slot_with_identical_bounds_reports_again() {
    let mut state = fresh_state();
    let first = area_opening_on(NestedMenuOpenTrigger::Hover);
    assert!(
        laid_out_report(
            &first,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(0.0)
        )
        .is_some()
    );
    let second = hover_area_with(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Hover);
    let _replaced = second.exit_of_replaced_target(&mut state);
    assert!(matches!(
        laid_out_report(
            &second,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(0.0)
        ),
        Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(
            WidgetAnchor {
                target: HoverTarget::Workspace(8),
                ..
            }
        )))
    ));
}

#[test]
fn a_replaced_target_exits_only_if_it_was_hovered() {
    let second = hover_area_with(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Hover);
    let mut unhovered = fresh_state();
    assert!(second.exit_of_replaced_target(&mut unhovered).is_none());
    let mut hovered = HoverState {
        pointer_presence: PointerPresence::PointedAt,
        ..fresh_state()
    };
    assert!(matches!(
        second.exit_of_replaced_target(&mut hovered),
        Some(HoverCardMessage::Exit {
            target: HoverTarget::Workspace(7)
        })
    ));
    assert_eq!(hovered.pointer_presence, PointerPresence::NotPointedAt);
    assert!(second.exit_of_replaced_target(&mut hovered).is_none());
}

#[test]
fn a_replaced_menu_handle_stops_being_pointed_at() {
    let second = hover_area_with(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Press);
    let mut hovered = HoverState {
        pointer_presence: PointerPresence::PointedAt,
        ..fresh_state()
    };
    assert!(matches!(
        second.exit_of_replaced_target(&mut hovered),
        Some(HoverCardMessage::Anchor(AnchorEvent::HandlePointed {
            target: HoverTarget::Workspace(7),
            pointing: PointerPresence::NotPointedAt,
            ..
        }))
    ));
}

#[test]
fn dropping_the_state_shows_in_the_reported_presence() {
    let area = area_opening_on(NestedMenuOpenTrigger::Hover);
    let mut state = fresh_state();
    let Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor))) = laid_out_report(
        &area,
        &mut state,
        PointerPresence::PointedAt,
        anchor_at(0.0),
    ) else {
        panic!("a hovered area reports");
    };
    assert_eq!(widget_anchor.widget_liveness(), WidgetLiveness::Alive);
    drop(state);
    assert_eq!(widget_anchor.widget_liveness(), WidgetLiveness::Dropped);
}

#[test]
fn a_replaced_target_no_longer_counts_as_the_old_widget() {
    let area = area_opening_on(NestedMenuOpenTrigger::Hover);
    let mut state = fresh_state();
    let Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor))) = laid_out_report(
        &area,
        &mut state,
        PointerPresence::PointedAt,
        anchor_at(0.0),
    ) else {
        panic!("a hovered area reports");
    };
    let replacement = hover_area_with(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Hover);
    let _exit = replacement.exit_of_replaced_target(&mut state);
    assert_eq!(widget_anchor.widget_liveness(), WidgetLiveness::Dropped);
}

#[test]
fn bounds_leaving_the_viewport_report_as_hidden_unless_hovered() {
    let area = area_opening_on(NestedMenuOpenTrigger::Hover);
    let mut state = fresh_state();
    let visibility = |message: Option<HoverCardMessage>| match message {
        Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor))) => {
            Some(widget_anchor.visibility)
        }
        _ => None,
    };
    assert_eq!(
        visibility(laid_out_report(
            &area,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(0.0)
        )),
        Some(AnchorVisibility::Visible)
    );
    assert_eq!(
        visibility(laid_out_report(
            &area,
            &mut state,
            PointerPresence::NotPointedAt,
            anchor_at(5000.0)
        )),
        Some(AnchorVisibility::Hidden)
    );
    assert_eq!(
        visibility(laid_out_report(
            &area,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(6000.0)
        )),
        Some(AnchorVisibility::Visible)
    );
}
