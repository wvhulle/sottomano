use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use sottomano_toml_config::indicator_groups::gamepad_bindings::{
    GamepadButton, GamepadStick, PointerButton, PointerSteeringSettings, PointerStickBinding,
    PointerStickSettings, StickDirection,
};

use super::{
    accelerometer_report_timing::ReportTiming,
    evdev_gamepad_devices::{ButtonEdge, ConnectedGamepad, PlaneAxis, RawGamepadEvent},
    stick_motion_tracking::{
        FlickTracker, PointerDisplacement, StickPosition, pointer_displacement,
    },
};
use crate::delivery_cadence_drivers::ChangeWatchFailure;

const POINTER_TICK: Duration = Duration::from_millis(8);
const LONGEST_POINTER_STEP: Duration = Duration::from_millis(32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotAccess {
    Allowed,
    Blocked,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PointerGate {
    pub compositor_driving_slots: BTreeSet<usize>,
}

impl PointerGate {
    #[must_use]
    pub fn slot_access(&self, slot: usize) -> SlotAccess {
        if self.compositor_driving_slots.contains(&slot) {
            SlotAccess::Allowed
        } else {
            SlotAccess::Blocked
        }
    }

    #[must_use]
    pub fn driving_slots(slots: impl IntoIterator<Item = usize>) -> Self {
        Self {
            compositor_driving_slots: slots.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct GamepadStreamSettings {
    pub steering: PointerSteeringSettings,
    pub gate: PointerGate,
}

#[derive(Debug, PartialEq)]
pub enum GamepadInput {
    DiscoveryFailed(ChangeWatchFailure),
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
        edge: ButtonEdge,
    },
}

#[derive(Debug, PartialEq)]
pub enum MachineEffect {
    Deliver(GamepadInput),
    Pointer(PointerEffect),
}

#[derive(Debug, PartialEq)]
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
    pointer_steering: PointerSteering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerSteering {
    Resting,
    SteppedAt(Instant),
}

impl GamepadMachine {
    #[must_use]
    pub fn new(settings: GamepadStreamSettings) -> Self {
        Self {
            settings,
            sticks: BTreeMap::new(),
            held_pointer_buttons: BTreeSet::new(),
            pointer_steering: PointerSteering::Resting,
        }
    }

    fn pointer_stick(&self) -> Option<GamepadStick> {
        match self.settings.steering.stick {
            PointerStickBinding::Bound(settings) => Some(settings.stick),
            PointerStickBinding::Unbound => None,
        }
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
            .filter(|(slot, _)| self.settings.gate.slot_access(**slot) == SlotAccess::Allowed)
            .filter_map(move |(_, sticks)| Some(sticks.tracker(pointer_stick?).position))
            .filter(|position| position.x != 0.0 || position.y != 0.0)
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let repeats = self.sticks.values().flat_map(|sticks| {
            self.flicking_sticks()
                .filter_map(|stick| sticks.tracker(stick).next_repeat())
        });
        let pointer_step = match self.pointer_steering {
            PointerSteering::SteppedAt(last_step) => Some(last_step + POINTER_TICK),
            PointerSteering::Resting => None,
        };
        repeats.chain(pointer_step).min()
    }

    pub fn step(&mut self, input: MachineInput, now: Instant) -> Vec<MachineEffect> {
        let effects = match input {
            MachineInput::Raw(RawGamepadEvent::DiscoveryFailed(watch_failure)) => {
                vec![MachineEffect::Deliver(GamepadInput::DiscoveryFailed(
                    watch_failure,
                ))]
            }
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
        if self.steering_positions().next().is_none() {
            self.pointer_steering = PointerSteering::Resting;
        } else if self.pointer_steering == PointerSteering::Resting {
            self.pointer_steering = PointerSteering::SteppedAt(now);
        }
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
                self.settings.gate.slot_access(slot) == SlotAccess::Allowed
                    && self.held_pointer_buttons.insert(held)
            }
            ButtonEdge::Released => {
                return self
                    .held_pointer_buttons
                    .remove(&held)
                    .then_some(PointerEffect::Button {
                        button: pointer_button,
                        edge,
                    });
            }
        };
        pressed.then_some(PointerEffect::Button {
            button: pointer_button,
            edge,
        })
    }

    fn move_stick(
        &mut self,
        slot: usize,
        stick: GamepadStick,
        axis: PlaneAxis,
        deflection: f32,
        now: Instant,
    ) -> Option<GamepadInput> {
        let pointer_stick = self.pointer_stick();
        let tracker = self.sticks.entry(slot).or_default().tracker_mut(stick);
        let direction = match axis {
            PlaneAxis::Horizontal => tracker.move_x(deflection, now),
            PlaneAxis::Vertical => tracker.move_y(deflection, now),
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
        let PointerSteering::SteppedAt(last_step) = self.pointer_steering else {
            return None;
        };
        let elapsed = now
            .checked_duration_since(last_step)
            .expect("the last pointer step happened at an earlier input");
        if elapsed < POINTER_TICK {
            return None;
        }
        let PointerStickBinding::Bound(PointerStickSettings {
            pixels_per_second, ..
        }) = self.settings.steering.stick
        else {
            return None;
        };
        let step = elapsed.min(LONGEST_POINTER_STEP);
        let displacement = self
            .steering_positions()
            .map(|position| pointer_displacement(position, pixels_per_second, step))
            .reduce(|total, more| PointerDisplacement {
                dx: total.dx + more.dx,
                dy: total.dy + more.dy,
            })?;
        self.pointer_steering = PointerSteering::SteppedAt(now);
        Some(PointerEffect::Move(displacement))
    }
}

#[cfg(test)]
mod gamepad_machine_tests;
