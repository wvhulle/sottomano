use std::{
    collections::{BTreeMap, BTreeSet},
    thread,
    time::{Duration, Instant},
};

use futures::Stream;
use gilrs::{Axis, Button, EventType, Gamepad, Gilrs, PowerInfo};
use sottomano_config::widget_groups::gamepad_bindings::{
    GamepadButton, GamepadStick, PointerButton, PointerSteeringSettings, StickDirection,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    stick_motion_tracking::{FlickTracker, pointer_displacement},
    wlr_virtual_pointer_motion::{PointerGate, VirtualPointer},
};

const ROSTER_REFRESH: Duration = Duration::from_mins(1);
const POINTER_TICK: Duration = Duration::from_millis(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadPower {
    Unknown,
    Wired,
    Discharging(u8),
    Charging(u8),
    Charged,
}

impl From<PowerInfo> for GamepadPower {
    fn from(power: PowerInfo) -> Self {
        match power {
            PowerInfo::Unknown => GamepadPower::Unknown,
            PowerInfo::Wired => GamepadPower::Wired,
            PowerInfo::Discharging(percent) => GamepadPower::Discharging(percent),
            PowerInfo::Charging(percent) => GamepadPower::Charging(percent),
            PowerInfo::Charged => GamepadPower::Charged,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedGamepad {
    pub slot: usize,
    pub name: String,
    pub vendor_id: Option<u16>,
    pub power: GamepadPower,
}

impl From<Gamepad<'_>> for ConnectedGamepad {
    fn from(gamepad: Gamepad<'_>) -> Self {
        Self {
            slot: gamepad.id().into(),
            name: gamepad.name().to_owned(),
            vendor_id: gamepad.vendor_id(),
            power: gamepad.power_info().into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEdge {
    Pressed,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamepadInput {
    PointerGate(PointerGate),
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
}

#[must_use]
pub fn standard_button(button: Button) -> Option<GamepadButton> {
    Some(match button {
        Button::South => GamepadButton::South,
        Button::East => GamepadButton::East,
        Button::North => GamepadButton::North,
        Button::West => GamepadButton::West,
        Button::DPadUp => GamepadButton::DpadUp,
        Button::DPadDown => GamepadButton::DpadDown,
        Button::DPadLeft => GamepadButton::DpadLeft,
        Button::DPadRight => GamepadButton::DpadRight,
        Button::LeftTrigger => GamepadButton::LeftBumper,
        Button::RightTrigger => GamepadButton::RightBumper,
        Button::LeftTrigger2 => GamepadButton::LeftTrigger,
        Button::RightTrigger2 => GamepadButton::RightTrigger,
        Button::Select => GamepadButton::Select,
        Button::Start => GamepadButton::Start,
        Button::Mode => GamepadButton::Mode,
        Button::LeftThumb => GamepadButton::LeftStick,
        Button::RightThumb => GamepadButton::RightStick,
        Button::C | Button::Z | Button::Unknown => return None,
    })
}

fn roster(gilrs: &Gilrs) -> GamepadInput {
    GamepadInput::Roster(
        gilrs
            .gamepads()
            .filter(|(_, gamepad)| gamepad.button_code(Button::South).is_some())
            .map(|(_, gamepad)| ConnectedGamepad::from(gamepad))
            .collect(),
    )
}

fn button_input(slot: usize, event: EventType) -> Option<GamepadInput> {
    let (button, edge) = match event {
        EventType::ButtonPressed(button, _) => (button, ButtonEdge::Pressed),
        EventType::ButtonReleased(button, _) => (button, ButtonEdge::Released),
        _ => return None,
    };
    Some(GamepadInput::Button {
        slot,
        button: standard_button(button)?,
        edge,
    })
}

struct SteeredPointer {
    pointer: VirtualPointer,
    pixels_per_second: u32,
    last_motion: Instant,
}

struct GamepadEventLoop {
    gilrs: Gilrs,
    inputs: mpsc::Sender<GamepadInput>,
    gate: PointerGate,
    left: FlickTracker,
    right: FlickTracker,
    pointer_stick: Option<GamepadStick>,
    pointer_buttons: BTreeMap<GamepadButton, PointerButton>,
    pressed_pointer_buttons: BTreeSet<PointerButton>,
    steered: Option<SteeredPointer>,
    last_slot: usize,
    next_roster: Instant,
}

impl GamepadEventLoop {
    fn new(
        gilrs: Gilrs,
        inputs: mpsc::Sender<GamepadInput>,
        steering: PointerSteeringSettings,
    ) -> Self {
        let pixels_per_second = steering
            .stick
            .map_or(0, |settings| settings.pixels_per_second);
        let steered = steering
            .needs_virtual_pointer()
            .then(VirtualPointer::connect)
            .and_then(|connected| match connected {
                Ok(pointer) => Some(SteeredPointer {
                    pointer,
                    pixels_per_second,
                    last_motion: Instant::now(),
                }),
                Err(e) => {
                    tracing::warn!("gamepad: pointer steering unavailable: {e}");
                    None
                }
            });
        Self {
            gilrs,
            inputs,
            gate: PointerGate::default(),
            left: FlickTracker::default(),
            right: FlickTracker::default(),
            pointer_stick: steering.stick.map(|settings| settings.stick),
            pointer_buttons: steering.buttons,
            pressed_pointer_buttons: BTreeSet::new(),
            steered,
            last_slot: 0,
            next_roster: Instant::now() + ROSTER_REFRESH,
        }
    }

    fn forward_pointer_button(&mut self, input: &GamepadInput) {
        let GamepadInput::Button { button, edge, .. } = *input else {
            return;
        };
        let Some(&pointer_button) = self.pointer_buttons.get(&button) else {
            return;
        };
        let pressed = edge == ButtonEdge::Pressed;
        if pressed {
            if !self.gate.is_allowed() {
                return;
            }
            self.pressed_pointer_buttons.insert(pointer_button);
        } else if !self.pressed_pointer_buttons.remove(&pointer_button) {
            return;
        }
        let Some(steered) = self.steered.as_mut() else {
            return;
        };
        if let Err(e) = steered.pointer.set_button(pointer_button, pressed) {
            tracing::warn!("gamepad: pointer steering stopped: {e}");
            self.steered = None;
        }
    }

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

    fn flicking_sticks(&self) -> impl Iterator<Item = GamepadStick> + use<> {
        let pointer_stick = self.pointer_stick;
        [GamepadStick::Left, GamepadStick::Right]
            .into_iter()
            .filter(move |&stick| Some(stick) != pointer_stick)
    }

    fn pointer_deadline(&self) -> Option<Instant> {
        let steered = self.steered.as_ref()?;
        let position = self.tracker(self.pointer_stick?).position();
        (position.is_deflected() && self.gate.is_allowed())
            .then_some(steered.last_motion + POINTER_TICK)
    }

    fn wait_budget(&self, now: Instant) -> Duration {
        self.flicking_sticks()
            .filter_map(|stick| self.tracker(stick).next_repeat())
            .chain([self.next_roster])
            .chain(self.pointer_deadline())
            .min()
            .map_or(ROSTER_REFRESH, |deadline| {
                deadline.saturating_duration_since(now)
            })
    }

    fn stick_flick(
        &mut self,
        slot: usize,
        axis: Axis,
        value: f32,
        now: Instant,
    ) -> Option<GamepadInput> {
        let stick = match axis {
            Axis::LeftStickX | Axis::LeftStickY => GamepadStick::Left,
            Axis::RightStickX | Axis::RightStickY => GamepadStick::Right,
            _ => return None,
        };
        let tracker = self.tracker_mut(stick);
        let direction = match axis {
            Axis::LeftStickX | Axis::RightStickX => tracker.move_x(value, now),
            _ => tracker.move_y(value, now),
        }?;
        (Some(stick) != self.pointer_stick).then_some(GamepadInput::StickFlick {
            slot,
            stick,
            direction,
        })
    }

    fn translate(&mut self, event: gilrs::Event, now: Instant) -> Option<GamepadInput> {
        let slot = event.id.into();
        self.last_slot = slot;
        match event.event {
            EventType::Connected | EventType::Disconnected => Some(roster(&self.gilrs)),
            EventType::AxisChanged(axis, value, _) => self.stick_flick(slot, axis, value, now),
            other => {
                let input = button_input(slot, other)?;
                self.forward_pointer_button(&input);
                Some(input)
            }
        }
    }

    fn due_inputs(&mut self, now: Instant) -> Vec<GamepadInput> {
        let slot = self.last_slot;
        let mut due: Vec<GamepadInput> = self
            .flicking_sticks()
            .filter_map(|stick| {
                let direction = self.tracker_mut(stick).repeat_due(now)?;
                Some(GamepadInput::StickFlick {
                    slot,
                    stick,
                    direction,
                })
            })
            .collect();
        if now >= self.next_roster {
            self.next_roster = now + ROSTER_REFRESH;
            due.push(roster(&self.gilrs));
        }
        due
    }

    fn steer_pointer(&mut self, now: Instant) {
        let Some(stick) = self.pointer_stick else {
            return;
        };
        let position = self.tracker(stick).position();
        let allowed = self.gate.is_allowed();
        let Some(steered) = self.steered.as_mut() else {
            return;
        };
        if !position.is_deflected() || !allowed {
            steered.last_motion = now;
            return;
        }
        let elapsed = now.duration_since(steered.last_motion);
        if elapsed < POINTER_TICK {
            return;
        }
        steered.last_motion = now;
        let displacement = pointer_displacement(
            position,
            steered.pixels_per_second,
            elapsed.min(POINTER_TICK * 4),
        );
        if let Err(e) = steered.pointer.move_by(displacement) {
            tracing::warn!("gamepad: pointer steering stopped: {e}");
            self.steered = None;
        }
    }

    fn run(mut self) {
        let opening = [
            GamepadInput::PointerGate(self.gate.clone()),
            roster(&self.gilrs),
        ];
        if opening
            .into_iter()
            .any(|input| self.inputs.blocking_send(input).is_err())
        {
            return;
        }
        loop {
            let budget = self.wait_budget(Instant::now());
            let event = self.gilrs.next_event_blocking(Some(budget));
            let now = Instant::now();
            let translated = event.and_then(|event| self.translate(event, now));
            let inputs = translated.into_iter().chain(self.due_inputs(now));
            for input in inputs {
                if self.inputs.blocking_send(input).is_err() {
                    return;
                }
            }
            self.steer_pointer(now);
        }
    }
}

pub fn gamepad_inputs(
    steering: PointerSteeringSettings,
) -> impl Stream<Item = GamepadInput> + Send {
    ReceiverStream::new(spawn_event_thread(steering))
}

fn spawn_event_thread(steering: PointerSteeringSettings) -> mpsc::Receiver<GamepadInput> {
    let (tx, rx) = mpsc::channel(64);
    thread::spawn(move || match Gilrs::new() {
        Ok(gilrs) => GamepadEventLoop::new(gilrs, tx, steering).run(),
        Err(e) => tracing::warn!("gamepad: input unavailable: {e}"),
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bumpers_and_triggers_follow_the_standard_layout_names() {
        assert_eq!(
            standard_button(Button::LeftTrigger),
            Some(GamepadButton::LeftBumper)
        );
        assert_eq!(
            standard_button(Button::LeftTrigger2),
            Some(GamepadButton::LeftTrigger)
        );
        assert_eq!(standard_button(Button::Unknown), None);
    }
}
