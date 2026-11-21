use iced::{
    Point, Settings, Size,
    mouse::{Button, Event as MouseEvent},
    widget::Space,
};
use iced_test::Simulator;
use sottomano_toml_config::screen_edges::Position;

use super::*;
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::named_test_indicator,
        indicator_gestures::IndicatorGesture,
        slider_control::{ControlMessage, SliderControl},
        toggle_control::{IconToggle, SwitchPresses},
    },
    group_menus::group_menu_state::{GroupMenuPin, GroupMenuTopic, PanelPin, PanelPinDuration},
    hover_cards::{
        AnchorEvent, HoverCardMessage, HoverTarget, IndicatorKey, MouseGesture, PopupMessage,
    },
    indicator_groups::{
        compositor_styling_group::CompositorStylingMessage,
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
};

fn composed_row<'indicator>() -> GroupIndicator<'indicator> {
    GroupIndicator::composed(
        "Composed row",
        IndicatorIcon::Absent,
        IndicatorValue::Nothing,
        Box::new(|_| Vec::new()),
    )
}

fn left_action(indicator: &GroupIndicator<'_>) -> Option<Cow<'static, str>> {
    indicator
        .press_actions()
        .into_iter()
        .find(|action| action.gesture == MouseGesture::LeftClick)
        .map(|action| action.does)
}

#[test]
fn composed_indicator_reports_its_identity_name() {
    assert_eq!(composed_row().indicator_name(), "Composed row");
}

#[test]
fn indicators_without_a_press_have_inert_gestures() {
    let gestures = composed_row().gestures.clone();
    assert!(matches!(gestures.left, IndicatorGesture::Inert));
    assert!(matches!(gestures.right, IndicatorGesture::Inert));
}

#[test]
fn every_listed_gesture_has_a_handler() {
    let indicator = composed_row()
        .on_press("left action", CompositorStylingMessage::ToggleXray)
        .on_right_press("right action", CompositorStylingMessage::ToggleXray);
    assert_eq!(
        indicator.press_actions(),
        vec![
            PressAction::new(MouseGesture::LeftClick, "left action"),
            PressAction::new(MouseGesture::RightClick, "right action"),
        ]
    );
    assert!(matches!(
        indicator.gestures.clone().right,
        IndicatorGesture::Performs { .. }
    ));
}

#[test]
fn declared_buttons_join_the_row_actions() {
    let indicator = composed_row().with_button_actions([PressAction::new(
        MouseGesture::LeftClick,
        "Next: skip to the next track",
    )]);
    assert_eq!(
        indicator.press_actions(),
        vec![PressAction::new(
            MouseGesture::LeftClick,
            "Next: skip to the next track"
        )]
    );
}

#[test]
fn a_composed_indicator_shares_the_pin_of_the_indicator_it_replaces_under_the_same_name() {
    let sample: GroupIndicator<'static> = named_test_indicator("Lyrics");
    let composed: GroupIndicator<'static> = GroupIndicator::composed_pinnable(
        "Lyrics",
        IndicatorIcon::Absent,
        IndicatorValue::Nothing,
        Box::new(|_| Vec::new()),
    );
    assert_eq!(composed.pinning, IndicatorPinning::Pinnable);
    assert_eq!(sample.indicator_name(), composed.indicator_name());
}

fn toggle_in_position(switch_position: SwitchPosition) -> IconToggle {
    IconToggle::new(
        "Toggle",
        Phosphor::Question(Weight::Regular),
        switch_position,
        SwitchPresses {
            turning_on: "turn on",
            turning_off: "turn off",
        },
    )
}

#[test]
fn an_indicator_with_only_a_press_action_shows_no_switch() {
    let launcher_row = composed_row().on_press("launch", CompositorStylingMessage::ToggleXray);
    assert_eq!(launcher_row.power_control, IndicatorPowerControl::Absent);
}

#[test]
fn a_toggle_shows_a_switch_in_its_current_position() {
    let indicator_of = |switch_position| {
        let indicator: GroupIndicator<'static> = GroupIndicator::toggle(
            toggle_in_position(switch_position),
            CompositorStylingMessage::ToggleXray,
        );
        indicator.power_control
    };
    assert_eq!(
        indicator_of(SwitchPosition::On),
        IndicatorPowerControl::Switch(SwitchPosition::On)
    );
    assert_eq!(
        indicator_of(SwitchPosition::Off),
        IndicatorPowerControl::Switch(SwitchPosition::Off)
    );
}

#[test]
fn a_switch_press_lists_the_same_gesture_as_a_plain_press() {
    let indicator = composed_row().on_switch_press(
        SwitchPosition::On,
        "turn off",
        CompositorStylingMessage::ToggleXray,
    );
    assert_eq!(left_action(&indicator).as_deref(), Some("turn off"));
}

fn offset_slider() -> SliderControl {
    SliderControl::new("Offset", -60.0..=60.0, Phosphor::Question(Weight::Regular))
}

#[test]
fn indicators_and_sliders_are_pinnable_but_plain_composed_rows_are_not() {
    let config = Config::default();
    let slider = offset_slider();
    let sample: GroupIndicator<'static> = named_test_indicator("Named reading");
    assert_eq!(sample.pinning, IndicatorPinning::Pinnable);
    assert_eq!(
        GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius).pinning,
        IndicatorPinning::Pinnable
    );
    assert_eq!(composed_row().pinning, IndicatorPinning::Unpinnable);
}

#[test]
fn a_slider_in_group_name_is_its_shown_name() {
    let config = Config::default();
    let slider = offset_slider();
    assert_eq!(
        GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius)
            .indicator_name(),
        slider.name.clone()
    );
}

#[test]
fn a_slider_lists_its_open_and_scroll_actions() {
    let config = Config::default();
    let slider = offset_slider();
    assert_eq!(
        GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius)
            .press_actions(),
        vec![
            PressAction::new(MouseGesture::LeftClick, "open the slider"),
            PressAction::new(MouseGesture::Scroll, "adjust"),
        ]
    );
}

fn pinned_slider_target() -> HoverTarget {
    HoverTarget::PinnedOnPanel(Box::new(PanelPin {
        menu: GroupMenuTopic::Media,
        entry: GroupMenuPin::Item(IndicatorKey {
            group: IndicatorGroupIdentity::sole(IndicatorGroupFamily::Media),
            name: "pinned slider".into(),
        }),
        duration: PanelPinDuration::Permanent,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderPointing {
    Hover,
    Press,
}

fn events_from_pointing_at_pinned_slider(slider_pointing: SliderPointing) -> Vec<GroupEvent> {
    let config = Config::default();
    let slider = offset_slider();
    let indicator =
        GroupIndicator::slider(&slider, &config, CompositorStylingMessage::CornerRadius);
    let mut panel = Simulator::with_size(
        Settings::default(),
        Size::new(200.0, 100.0),
        indicator.view(
            &config,
            1.0,
            Clock::real(),
            pinned_slider_target(),
            Space::new().into(),
            Position::Top,
        ),
    );
    let inside_slider = Point::new(5.0, 50.0);
    panel.point_at(inside_slider);
    let _moved_inside = panel.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
        position: inside_slider,
    })]);
    if slider_pointing == SliderPointing::Press {
        let _pressed =
            panel.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
    }
    panel.into_messages().collect()
}

#[test]
fn hovering_a_pinned_slider_enters_its_one_pinned_hover_area() {
    let entered_targets: Vec<HoverTarget> =
        events_from_pointing_at_pinned_slider(SliderPointing::Hover)
            .into_iter()
            .filter_map(|event| match event {
                GroupEvent::Popup(PopupMessage::HoverCard(HoverCardMessage::Anchor(
                    AnchorEvent::Entered { target },
                ))) => Some(target),
                _ => None,
            })
            .collect();
    assert_eq!(entered_targets, vec![pinned_slider_target()]);
}

#[test]
fn pressing_a_pinned_slider_toggles_it() {
    assert!(
        events_from_pointing_at_pinned_slider(SliderPointing::Press)
            .iter()
            .any(|event| matches!(
                event,
                GroupEvent::Interaction(IndicatorGroupInteraction::CompositorStyling(
                    CompositorStylingMessage::CornerRadius(ControlMessage::Press)
                ))
            ))
    );
}

#[test]
fn a_stop_press_shows_a_stop_glyph_and_an_inert_switch_has_no_press() {
    let stoppable =
        composed_row().on_stop_press("stop the machine", CompositorStylingMessage::ToggleXray);
    assert_eq!(stoppable.power_control, IndicatorPowerControl::StopGlyph);
    assert_eq!(left_action(&stoppable).as_deref(), Some("stop the machine"));
    let uncontrolled = composed_row().inert_switch(SwitchPosition::On);
    assert_eq!(
        uncontrolled.power_control,
        IndicatorPowerControl::InertSwitch(SwitchPosition::On)
    );
    assert_eq!(left_action(&uncontrolled), None);
}
