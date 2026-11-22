use sottomano_toml_config::indicator_groups::gamepad_bindings::GamepadBindingsConfig;

use super::*;

fn machine_with(gate: PointerGate) -> GamepadMachine {
    GamepadMachine::new(GamepadStreamSettings {
        steering: GamepadBindingsConfig::default().pointer_steering(),
        gate,
    })
}

fn right_stick_right(slot: usize) -> MachineInput {
    MachineInput::Raw(RawGamepadEvent::Stick {
        slot,
        stick: GamepadStick::Right,
        axis: PlaneAxis::Horizontal,
        deflection: 1.0,
    })
}

#[test]
fn a_deflected_pointer_stick_moves_the_pointer_on_the_next_tick() {
    let start = Instant::now();
    let mut machine = machine_with(PointerGate::driving_slots([31]));
    assert!(machine.step(right_stick_right(31), start).is_empty());
    let deadline = machine.next_deadline().unwrap();
    let effects = machine.step(MachineInput::Tick, deadline);
    assert!(matches!(
        effects.as_slice(),
        [MachineEffect::Pointer(PointerEffect::Move(displacement))] if displacement.dx > 0.0
    ));
}

#[test]
fn a_controller_without_compositor_control_leaves_the_pointer_alone() {
    let start = Instant::now();
    let mut machine = machine_with(PointerGate::default());
    machine.step(right_stick_right(31), start);
    assert_eq!(machine.next_deadline(), None);
}

#[test]
fn only_the_driving_controller_steers_the_pointer() {
    let start = Instant::now();
    let mut machine = machine_with(PointerGate::driving_slots([32]));
    machine.step(right_stick_right(31), start);
    assert_eq!(machine.next_deadline(), None);
    machine.step(right_stick_right(32), start);
    assert!(machine.next_deadline().is_some());
}

#[test]
fn the_right_trigger_presses_and_releases_the_left_mouse_button() {
    let start = Instant::now();
    let mut machine = machine_with(PointerGate::driving_slots([31]));
    let trigger = |edge| {
        MachineInput::Raw(RawGamepadEvent::Button {
            slot: 31,
            button: GamepadButton::RightTrigger,
            edge,
        })
    };
    assert_eq!(
        machine.step(trigger(ButtonEdge::Pressed), start)[0],
        MachineEffect::Pointer(PointerEffect::Button {
            button: PointerButton::Left,
            edge: ButtonEdge::Pressed,
        })
    );
    assert_eq!(
        machine.step(trigger(ButtonEdge::Released), start)[0],
        MachineEffect::Pointer(PointerEffect::Button {
            button: PointerButton::Left,
            edge: ButtonEdge::Released,
        })
    );
}

#[test]
fn the_left_stick_flicks_while_the_right_stick_only_steers() {
    let start = Instant::now();
    let mut machine = machine_with(PointerGate::default());
    let effects = machine.step(
        MachineInput::Raw(RawGamepadEvent::Stick {
            slot: 31,
            stick: GamepadStick::Left,
            axis: PlaneAxis::Horizontal,
            deflection: -0.9,
        }),
        start,
    );
    assert_eq!(
        effects,
        [MachineEffect::Deliver(GamepadInput::StickFlick {
            slot: 31,
            stick: GamepadStick::Left,
            direction: StickDirection::Left,
        })]
    );
}
