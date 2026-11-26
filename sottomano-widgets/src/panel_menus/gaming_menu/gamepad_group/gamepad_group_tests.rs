use sottomano_desktop_sources::gamepad_controller_input::{
    evdev_gamepad_devices::{GamepadPower, InputRoute, UsbCableState},
    player_indicator_leds::{LitPlayer, PlayerIndicator},
};
use sottomano_toml_config::indicator_groups::gamepad_bindings::{GamepadStick, StickDirection};

use super::{
    button_role_actions::typed_key_command, connected_controller_indicator::ControllerMarking, *,
};
use crate::{clock::Clock, group_indicators::indicator_group_identity::IndicatorGroupTitle};

const DUALSENSE: usize = 31;
const SECOND_PAD: usize = 32;
const STEAM_VIRTUAL_PAD: usize = 261;

fn button(slot: usize, button: GamepadButton, edge: ButtonEdge) -> GamepadMessage {
    GamepadMessage::Input(GamepadInput::Button { slot, button, edge })
}

fn press(group: &mut GamepadGroup, slot: usize, pressed: GamepadButton) -> Action {
    group.update(
        button(slot, pressed, ButtonEdge::Pressed),
        &GamepadBindingsConfig::default(),
    )
}

fn release(group: &mut GamepadGroup, slot: usize, released: GamepadButton) -> Action {
    group.update(
        button(slot, released, ButtonEdge::Released),
        &GamepadBindingsConfig::default(),
    )
}

fn toggle_compositor_driving(group: &mut GamepadGroup, slot: usize) -> Action {
    group.interact(GamepadInteraction::ToggleCompositorDriving { slot })
}

fn spawned_command(action: Action) -> Vec<String> {
    let Action::Compositor(CompositorRequest::Spawn(command)) = action else {
        panic!("expected a spawned command, got {action:?}");
    };
    command
}

fn connected_pad(slot: usize, origin: GamepadOrigin) -> ConnectedGamepad {
    ConnectedGamepad {
        slot,
        name: "pad".to_owned(),
        vendor_id: 0x054c,
        product_id: 0x0ce6,
        power: GamepadPower::NoBattery,
        origin,
    }
}

fn lit_player(number: u8) -> PlayerIndicator {
    PlayerIndicator::Lit(LitPlayer {
        number,
        leds_lit: Vec::new(),
    })
}

fn bluetooth_link(slot: usize, player_indicator: PlayerIndicator) -> GamepadOrigin {
    GamepadOrigin::Physical(PhysicalGamepadLink {
        player_indicator,
        kernel_device: format!("+hid:{slot}"),
        transport: GamepadTransport::Bluetooth {
            usb_cable: UsbCableState::Absent,
        },
        identity: ControllerIdentity::Addressed {
            hardware_address: format!("00:00:00:00:00:{slot:02x}"),
            alias: format!("Pad {slot}"),
        },
        input_route: InputRoute::Kernel,
    })
}

fn group_with_two_pads_and_a_virtual_one() -> GamepadGroup {
    let mut group = GamepadGroup::default();
    group.update(
        GamepadMessage::Input(GamepadInput::Roster(vec![
            connected_pad(DUALSENSE, bluetooth_link(DUALSENSE, lit_player(2))),
            connected_pad(SECOND_PAD, bluetooth_link(SECOND_PAD, lit_player(1))),
            connected_pad(STEAM_VIRTUAL_PAD, GamepadOrigin::Virtual),
        ])),
        &GamepadBindingsConfig::default(),
    );
    group
}

fn group_with_the_dualsense_driving_the_compositor() -> GamepadGroup {
    let mut group = group_with_two_pads_and_a_virtual_one();
    toggle_compositor_driving(&mut group, DUALSENSE);
    group
}

#[test]
fn dpad_left_types_the_left_arrow_key_by_default() {
    let action = press(
        &mut group_with_the_dualsense_driving_the_compositor(),
        DUALSENSE,
        GamepadButton::DpadLeft,
    );
    assert_eq!(spawned_command(action), ["wtype", "-k", "Left"]);
}

#[test]
fn triangle_types_shift_tab_by_default() {
    let action = press(
        &mut group_with_the_dualsense_driving_the_compositor(),
        DUALSENSE,
        GamepadButton::North,
    );
    assert_eq!(
        spawned_command(action),
        ["wtype", "-M", "shift", "-k", "Tab", "-m", "shift"]
    );
}

#[test]
fn two_modifiers_are_released_in_reverse_order() {
    assert_eq!(
        typed_key_command("ctrl+shift+Tab"),
        [
            "wtype", "-M", "ctrl", "-M", "shift", "-k", "Tab", "-m", "shift", "-m", "ctrl"
        ]
    );
}

#[test]
fn flicking_the_left_stick_right_focuses_to_the_right() {
    let action = group_with_the_dualsense_driving_the_compositor().update(
        GamepadMessage::Input(GamepadInput::StickFlick {
            slot: DUALSENSE,
            stick: GamepadStick::Left,
            direction: StickDirection::Right,
        }),
        &GamepadBindingsConfig::default(),
    );
    assert!(matches!(
        action,
        Action::Compositor(CompositorRequest::FocusRight)
    ));
}

#[test]
fn holding_the_left_trigger_dictates_until_released() {
    let mut group = group_with_the_dualsense_driving_the_compositor();
    let started = press(&mut group, DUALSENSE, GamepadButton::LeftTrigger);
    assert!(matches!(
        started,
        Action::Dictation(DictationCommand::Start)
    ));
    let stopped = release(&mut group, DUALSENSE, GamepadButton::LeftTrigger);
    assert!(matches!(stopped, Action::Dictation(DictationCommand::Stop)));
}

#[test]
fn controllers_start_out_handed_to_the_focused_window() {
    let mut group = group_with_two_pads_and_a_virtual_one();
    for slot in [DUALSENSE, SECOND_PAD, STEAM_VIRTUAL_PAD] {
        assert_eq!(
            group.controller_custody(slot),
            ControllerCustody::HandedOver
        );
        assert!(matches!(
            press(&mut group, slot, GamepadButton::DpadLeft),
            Action::Idle
        ));
    }
    assert!(
        group
            .stream_settings(&GamepadBindingsConfig::default())
            .gate
            .compositor_driving_slots
            .is_empty()
    );
}

#[test]
fn an_unknown_slot_never_drives_the_compositor() {
    let action = GamepadGroup::default().update(
        GamepadMessage::Input(GamepadInput::StickFlick {
            slot: DUALSENSE,
            stick: GamepadStick::Left,
            direction: StickDirection::Right,
        }),
        &GamepadBindingsConfig::default(),
    );
    assert!(matches!(action, Action::Idle));
}

#[test]
fn switching_on_one_controller_leaves_the_others_with_the_focused_window() {
    let mut group = group_with_two_pads_and_a_virtual_one();
    let saved = toggle_compositor_driving(&mut group, DUALSENSE);
    assert!(matches!(
        saved,
        Action::SaveDrivingControllers(driving_controllers)
            if driving_controllers == BTreeSet::from(["00:00:00:00:00:1f".to_owned()])
    ));
    assert!(matches!(
        press(&mut group, DUALSENSE, GamepadButton::DpadLeft),
        Action::Compositor(_)
    ));
    assert!(matches!(
        press(&mut group, SECOND_PAD, GamepadButton::DpadLeft),
        Action::Idle
    ));
    assert_eq!(
        group
            .stream_settings(&GamepadBindingsConfig::default())
            .gate
            .compositor_driving_slots,
        BTreeSet::from([DUALSENSE])
    );
    toggle_compositor_driving(&mut group, DUALSENSE);
    assert!(matches!(
        press(&mut group, DUALSENSE, GamepadButton::DpadLeft),
        Action::Idle
    ));
}

#[test]
fn a_virtual_pad_cannot_be_switched_on() {
    let mut group = group_with_two_pads_and_a_virtual_one();
    assert!(matches!(
        toggle_compositor_driving(&mut group, STEAM_VIRTUAL_PAD),
        Action::Idle
    ));
    assert_eq!(
        group.controller_custody(STEAM_VIRTUAL_PAD),
        ControllerCustody::HandedOver
    );
}

#[test]
fn compositor_control_survives_the_controller_reconnecting_on_a_new_slot() {
    let mut group = group_with_the_dualsense_driving_the_compositor();
    let reconnected_slot = 40;
    group.update(
        GamepadMessage::Input(GamepadInput::Roster(vec![connected_pad(
            reconnected_slot,
            bluetooth_link(DUALSENSE, lit_player(1)),
        )])),
        &GamepadBindingsConfig::default(),
    );
    assert_eq!(
        group.controller_custody(reconnected_slot),
        ControllerCustody::Compositor
    );
}

#[test]
fn loaded_driving_controllers_take_effect_for_connected_pads() {
    let mut group = group_with_two_pads_and_a_virtual_one();
    group.update(
        GamepadMessage::DrivingControllersLoaded(BTreeSet::from(["00:00:00:00:00:20".to_owned()])),
        &GamepadBindingsConfig::default(),
    );
    assert_eq!(
        group.controller_custody(SECOND_PAD),
        ControllerCustody::Compositor
    );
    assert_eq!(
        group.controller_custody(DUALSENSE),
        ControllerCustody::HandedOver
    );
}

#[test]
fn controllers_follow_their_lit_player_number() {
    let group = group_with_two_pads_and_a_virtual_one();
    let ordered_slots: Vec<usize> = group
        .controllers_in_player_order()
        .into_iter()
        .map(|(controller, _)| controller.slot)
        .collect();
    assert_eq!(ordered_slots, [SECOND_PAD, DUALSENSE]);
}

#[test]
fn each_physical_pad_gets_its_own_group() {
    let group = group_with_two_pads_and_a_virtual_one();
    assert_eq!(
        group
            .group_contents(&GamepadBindingsConfig::default(), Clock::real())
            .len(),
        2
    );
}

fn pad_marking_in_slot(group: &GamepadGroup, slot: usize) -> ControllerMarking<'_> {
    group.controller_marking(
        group
            .controllers
            .iter()
            .find(|controller| controller.slot == slot)
            .unwrap(),
        group.physical_link(slot).unwrap(),
    )
}

#[test]
fn a_lit_pad_is_marked_by_its_player_number_instead_of_its_alias() {
    let group = group_with_two_pads_and_a_virtual_one();
    assert!(matches!(
        pad_marking_in_slot(&group, DUALSENSE),
        ControllerMarking::PlayerLights(LitPlayer { number: 2, .. })
    ));
    assert!(matches!(
        pad_marking_in_slot(&group, SECOND_PAD),
        ControllerMarking::PlayerLights(LitPlayer { number: 1, .. })
    ));
}

#[test]
fn unlit_pads_fall_back_to_their_alias_only_when_ambiguous() {
    let mut group = GamepadGroup::default();
    let roster = |pads: Vec<ConnectedGamepad>| GamepadMessage::Input(GamepadInput::Roster(pads));
    let unlit_pad = |slot| connected_pad(slot, bluetooth_link(slot, PlayerIndicator::Unlit));
    group.update(
        roster(vec![unlit_pad(DUALSENSE)]),
        &GamepadBindingsConfig::default(),
    );
    assert!(matches!(
        pad_marking_in_slot(&group, DUALSENSE),
        ControllerMarking::IconOnly
    ));
    group.update(
        roster(vec![unlit_pad(DUALSENSE), unlit_pad(SECOND_PAD)]),
        &GamepadBindingsConfig::default(),
    );
    assert!(matches!(
        pad_marking_in_slot(&group, DUALSENSE),
        ControllerMarking::PairedAlias("Pad 31")
    ));
}

#[test]
fn namesake_controllers_get_numbered_titles() {
    let titles: Vec<String> =
        IndicatorGroupTitle::numbered_namesakes(["DualSense", "Xbox", "DualSense"])
            .iter()
            .map(ToString::to_string)
            .collect();
    assert_eq!(titles, ["DualSense", "Xbox", "DualSense 2"]);
}
