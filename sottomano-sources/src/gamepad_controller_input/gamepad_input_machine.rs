use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use sottomano_config::widget_groups::gamepad_bindings::{
    GamepadButton, GamepadStick, PointerButton, PointerSteeringSettings, StickDirection,
};

use super::{
    accelerometer_report_timing::ReportTiming,
    evdev_gamepad_devices::{ButtonEdge, ConnectedGamepad, RawGamepadEvent, StickAxis},
    stick_motion_tracking::{
        FlickTracker, PointerDisplacement, StickPosition, pointer_displacement,
    },
};

const POINTER_TICK: Duration = Duration::from_millis(8);
const LONGEST_POINTER_STEP: Duration = Duration::from_millis(32);

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PointerGate {
    pub silenced_by_focus: bool,
    pub handed_over_slots: BTreeSet<usize>,
}

impl PointerGate {
    #[must_use]
    pub fn allows(&self, slot: usize) -> bool {
        !self.silenced_by_focus && !self.handed_over_slots.contains(&slot)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct GamepadStreamSettings {
    pub steering: PointerSteeringSettings,
    pub gate: PointerGate,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GamepadInput {
    Roster(Vec<ConnectedGamepad>),
    Button {
        slot: usize,
        button: GamepadButton,
        edge: ButtonEdge,
    },
    StickFlick {
        slot: usize,
        stick: GamepadStick,
        direction: StickDirection,
    },
    ReportTiming {
        slot: usize,
        timing: ReportTiming,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerEffect {
    Move(PointerDisplacement),
    Button {
        button: PointerButton,
        pressed: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MachineEffect {
    Deliver(GamepadInput),
    Pointer(PointerEffect),
}

#[derive(Debug, Clone, PartialEq)]
pub enum MachineInput {
    Raw(RawGamepadEvent),
    Tick,
}

#[derive(Debug, Clone, Default)]
struct SlotSticks {
    left: FlickTracker,
    right: FlickTracker,
}

impl SlotSticks {
    fn tracker(&self, stick: GamepadStick) -> &FlickTracker {
        match stick {
            GamepadStick::Left => &self.left,
            GamepadStick::Right => &self.right,
        }
    }

    fn tracker_mut(&mut self, stick: GamepadStick) -> &mut FlickTracker {
        match stick {
            GamepadStick::Left => &mut self.left,
            GamepadStick::Right => &mut self.right,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct HeldPointerButton {
    slot: usize,
    button: PointerButton,
}

#[derive(Debug, Clone)]
pub struct GamepadMachine {
    settings: GamepadStreamSettings,
    sticks: BTreeMap<usize, SlotSticks>,
    held_pointer_buttons: BTreeSet<HeldPointerButton>,
    last_pointer_step: Option<Instant>,
}

impl GamepadMachine {
    #[must_use]
    pub fn new(settings: GamepadStreamSettings) -> Self {
        Self {
            settings,
            sticks: BTreeMap::new(),
            held_pointer_buttons: BTreeSet::new(),
            last_pointer_step: None,
        }
    }

    fn pointer_stick(&self) -> Option<GamepadStick> {
        self.settings.steering.stick.map(|settings| settings.stick)
    }

    fn flicking_sticks(&self) -> impl Iterator<Item = GamepadStick> + use<> {
        let pointer_stick = self.pointer_stick();
        [GamepadStick::Left, GamepadStick::Right]
            .into_iter()
            .filter(move |&stick| Some(stick) != pointer_stick)
    }

    fn steering_positions(&self) -> impl Iterator<Item = StickPosition> + '_ {
        let pointer_stick = self.pointer_stick();
        self.sticks
            .iter()
            .filter(|(slot, _)| self.settings.gate.allows(**slot))
            .filter_map(move |(_, sticks)| Some(sticks.tracker(pointer_stick?).position()))
            .filter(|position| position.is_deflected())
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let repeats = self.sticks.values().flat_map(|sticks| {
            self.flicking_sticks()
                .filter_map(|stick| sticks.tracker(stick).next_repeat())
        });
        let pointer_step = self
            .last_pointer_step
            .map(|last_step| last_step + POINTER_TICK);
        repeats.chain(pointer_step).min()
    }

    pub fn step(&mut self, input: MachineInput, now: Instant) -> Vec<MachineEffect> {
        let effects = match input {
            MachineInput::Raw(RawGamepadEvent::Roster(roster)) => {
                self.sticks
                    .retain(|slot, _| roster.iter().any(|gamepad| gamepad.slot == *slot));
                vec![MachineEffect::Deliver(GamepadInput::Roster(roster))]
            }
            MachineInput::Raw(RawGamepadEvent::Button { slot, button, edge }) => self
                .pointer_button(slot, button, edge)
                .map(MachineEffect::Pointer)
                .into_iter()
                .chain([MachineEffect::Deliver(GamepadInput::Button {
                    slot,
                    button,
                    edge,
                })])
                .collect(),
            MachineInput::Raw(RawGamepadEvent::Stick {
                slot,
                stick,
                axis,
                deflection,
            }) => self
                .move_stick(slot, stick, axis, deflection, now)
                .map(MachineEffect::Deliver)
                .into_iter()
                .collect(),
            MachineInput::Raw(RawGamepadEvent::ReportTiming { slot, timing }) => {
                vec![MachineEffect::Deliver(GamepadInput::ReportTiming {
                    slot,
                    timing,
                })]
            }
            MachineInput::Tick => self
                .due_flick_repeats(now)
                .into_iter()
                .map(MachineEffect::Deliver)
                .chain(self.pointer_step(now).map(MachineEffect::Pointer))
                .collect(),
        };
        let steering = self.steering_positions().next().is_some();
        self.last_pointer_step = steering.then(|| self.last_pointer_step.unwrap_or(now));
        effects
    }

    fn pointer_button(
        &mut self,
        slot: usize,
        button: GamepadButton,
        edge: ButtonEdge,
    ) -> Option<PointerEffect> {
        let pointer_button = *self.settings.steering.buttons.get(&button)?;
        let held = HeldPointerButton {
            slot,
            button: pointer_button,
        };
        let pressed = match edge {
            ButtonEdge::Pressed => {
                self.settings.gate.allows(slot) && self.held_pointer_buttons.insert(held)
            }
            ButtonEdge::Released => {
                return self
                    .held_pointer_buttons
                    .remove(&held)
                    .then_some(PointerEffect::Button {
                        button: pointer_button,
                        pressed: false,
                    });
            }
        };
        pressed.then_some(PointerEffect::Button {
            button: pointer_button,
            pressed: true,
        })
    }

    fn move_stick(
        &mut self,
        slot: usize,
        stick: GamepadStick,
        axis: StickAxis,
        deflection: f32,
        now: Instant,
    ) -> Option<GamepadInput> {
        let pointer_stick = self.pointer_stick();
        let tracker = self.sticks.entry(slot).or_default().tracker_mut(stick);
        let direction = match axis {
            StickAxis::Horizontal => tracker.move_x(deflection, now),
            StickAxis::Vertical => tracker.move_y(deflection, now),
        }?;
        (Some(stick) != pointer_stick).then_some(GamepadInput::StickFlick {
            slot,
            stick,
            direction,
        })
    }

    fn due_flick_repeats(&mut self, now: Instant) -> Vec<GamepadInput> {
        let flicking: Vec<GamepadStick> = self.flicking_sticks().collect();
        self.sticks
            .iter_mut()
            .flat_map(|(&slot, sticks)| {
                flicking
                    .iter()
                    .filter_map(|&stick| {
                        let direction = sticks.tracker_mut(stick).repeat_due(now)?;
                        Some(GamepadInput::StickFlick {
                            slot,
                            stick,
                            direction,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn pointer_step(&mut self, now: Instant) -> Option<PointerEffect> {
        let last_step = self.last_pointer_step?;
        let elapsed = now.duration_since(last_step);
        if elapsed < POINTER_TICK {
            return None;
        }
        let pixels_per_second = self.settings.steering.stick?.pixels_per_second;
        let step = elapsed.min(LONGEST_POINTER_STEP);
        let displacement = self
            .steering_positions()
            .map(|position| pointer_displacement(position, pixels_per_second, step))
            .reduce(|total, more| PointerDisplacement {
                dx: total.dx + more.dx,
                dy: total.dy + more.dy,
            })?;
        self.last_pointer_step = Some(now);
        Some(PointerEffect::Move(displacement))
    }
}

#[cfg(test)]
mod tests {
    use sottomano_config::widget_groups::gamepad_bindings::GamepadBindingsConfig;

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
            axis: StickAxis::Horizontal,
            deflection: 1.0,
        })
    }

    #[test]
    fn a_deflected_pointer_stick_moves_the_pointer_on_the_next_tick() {
        let start = Instant::now();
        let mut machine = machine_with(PointerGate::default());
        assert!(machine.step(right_stick_right(31), start).is_empty());
        let deadline = machine.next_deadline().unwrap();
        let effects = machine.step(MachineInput::Tick, deadline);
        assert!(matches!(
            effects.as_slice(),
            [MachineEffect::Pointer(PointerEffect::Move(displacement))] if displacement.dx > 0.0
        ));
    }

    #[test]
    fn a_focused_game_leaves_the_pointer_alone() {
        let start = Instant::now();
        let mut machine = machine_with(PointerGate {
            silenced_by_focus: true,
            ..PointerGate::default()
        });
        machine.step(right_stick_right(31), start);
        assert_eq!(machine.next_deadline(), None);
    }

    #[test]
    fn a_handed_over_controller_leaves_the_pointer_alone_while_others_steer() {
        let start = Instant::now();
        let mut machine = machine_with(PointerGate {
            handed_over_slots: BTreeSet::from([31]),
            ..PointerGate::default()
        });
        machine.step(right_stick_right(31), start);
        assert_eq!(machine.next_deadline(), None);
        machine.step(right_stick_right(32), start);
        assert!(machine.next_deadline().is_some());
    }

    #[test]
    fn the_right_trigger_presses_and_releases_the_left_mouse_button() {
        let start = Instant::now();
        let mut machine = machine_with(PointerGate::default());
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
                pressed: true,
            })
        );
        assert_eq!(
            machine.step(trigger(ButtonEdge::Released), start)[0],
            MachineEffect::Pointer(PointerEffect::Button {
                button: PointerButton::Left,
                pressed: false,
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
                axis: StickAxis::Horizontal,
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
}
