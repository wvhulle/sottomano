use std::collections::HashMap;

use evdev::{AbsoluteAxisCode, Device, EventSummary, InputEvent, KeyCode};
use sottomano_toml_config::panel_menus::gaming_menu::gamepad_bindings::{
    GamepadButton, GamepadStick,
};

use super::{ButtonEdge, PlaneAxis, RawGamepadEvent};

const STICK_DEADZONE: f32 = 0.12;
const ANALOG_TRIGGER_ENGAGE: f32 = 0.5;
const ANALOG_TRIGGER_RELEASE: f32 = 0.3;

fn standard_button(key: KeyCode) -> Option<GamepadButton> {
    Some(match key {
        KeyCode::BTN_SOUTH => GamepadButton::South,
        KeyCode::BTN_EAST => GamepadButton::East,
        KeyCode::BTN_NORTH => GamepadButton::North,
        KeyCode::BTN_WEST => GamepadButton::West,
        KeyCode::BTN_DPAD_UP => GamepadButton::DpadUp,
        KeyCode::BTN_DPAD_DOWN => GamepadButton::DpadDown,
        KeyCode::BTN_DPAD_LEFT => GamepadButton::DpadLeft,
        KeyCode::BTN_DPAD_RIGHT => GamepadButton::DpadRight,
        KeyCode::BTN_TL => GamepadButton::LeftShoulder,
        KeyCode::BTN_TR => GamepadButton::RightShoulder,
        KeyCode::BTN_TL2 => GamepadButton::LeftTrigger,
        KeyCode::BTN_TR2 => GamepadButton::RightTrigger,
        KeyCode::BTN_SELECT => GamepadButton::Back,
        KeyCode::BTN_START => GamepadButton::Start,
        KeyCode::BTN_MODE => GamepadButton::Guide,
        KeyCode::BTN_THUMBL => GamepadButton::LeftStick,
        KeyCode::BTN_THUMBR => GamepadButton::RightStick,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisRange {
    pub minimum: i32,
    pub maximum: i32,
}

impl AxisRange {
    fn centered_deflection(self, raw: i32) -> f32 {
        let center = f64::midpoint(f64::from(self.minimum), f64::from(self.maximum));
        let half_span = (f64::from(self.maximum) - f64::from(self.minimum)) / 2.0;
        let deflection = ((f64::from(raw) - center) / half_span).clamp(-1.0, 1.0) as f32;
        if deflection.abs() < STICK_DEADZONE {
            0.0
        } else {
            deflection
        }
    }

    fn pull(self, raw: i32) -> f32 {
        let span = f64::from(self.maximum) - f64::from(self.minimum);
        ((f64::from(raw) - f64::from(self.minimum)) / span).clamp(0.0, 1.0) as f32
    }
}

impl PlaneAxis {
    const fn hat_button(self, value: i32) -> Option<GamepadButton> {
        match (self, value.signum()) {
            (PlaneAxis::Horizontal, -1) => Some(GamepadButton::DpadLeft),
            (PlaneAxis::Horizontal, 1) => Some(GamepadButton::DpadRight),
            (PlaneAxis::Vertical, -1) => Some(GamepadButton::DpadUp),
            (PlaneAxis::Vertical, 1) => Some(GamepadButton::DpadDown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GamepadEventTranslator {
    slot: usize,
    ranges: HashMap<AbsoluteAxisCode, AxisRange>,
    trigger_kind: TriggerKind,
    hat_positions: HashMap<AbsoluteAxisCode, i32>,
    analog_triggers_held: HashMap<AbsoluteAxisCode, bool>,
}

impl GamepadEventTranslator {
    #[must_use]
    pub fn new(
        slot: usize,
        ranges: HashMap<AbsoluteAxisCode, AxisRange>,
        trigger_kind: TriggerKind,
    ) -> Self {
        Self {
            slot,
            ranges,
            trigger_kind,
            ..Self::default()
        }
    }

    pub(super) fn probe_device_axes(slot: usize, device: &Device) -> Self {
        let ranges = device
            .get_absinfo()
            .into_iter()
            .flatten()
            .map(|(axis, info)| {
                (
                    axis,
                    AxisRange {
                        minimum: info.minimum(),
                        maximum: info.maximum(),
                    },
                )
            })
            .collect();
        let trigger_kind = if device
            .supported_keys()
            .is_some_and(|keys| keys.contains(KeyCode::BTN_TL2))
        {
            TriggerKind::Digital
        } else {
            TriggerKind::Analog
        };
        Self::new(slot, ranges, trigger_kind)
    }

    fn button(&self, button: GamepadButton, edge: ButtonEdge) -> RawGamepadEvent {
        RawGamepadEvent::Button {
            slot: self.slot,
            button,
            edge,
        }
    }

    fn stick(&self, axis: AbsoluteAxisCode, raw: i32) -> Option<RawGamepadEvent> {
        let (stick, stick_axis, sign) = match axis {
            AbsoluteAxisCode::ABS_X => (GamepadStick::Left, PlaneAxis::Horizontal, 1.0),
            AbsoluteAxisCode::ABS_Y => (GamepadStick::Left, PlaneAxis::Vertical, -1.0),
            AbsoluteAxisCode::ABS_RX => (GamepadStick::Right, PlaneAxis::Horizontal, 1.0),
            AbsoluteAxisCode::ABS_RY => (GamepadStick::Right, PlaneAxis::Vertical, -1.0),
            _ => return None,
        };
        let range = self.ranges.get(&axis)?;
        Some(RawGamepadEvent::Stick {
            slot: self.slot,
            stick,
            axis: stick_axis,
            deflection: sign * range.centered_deflection(raw),
        })
    }

    fn hat(
        &mut self,
        axis: AbsoluteAxisCode,
        hat_axis: PlaneAxis,
        value: i32,
    ) -> Vec<RawGamepadEvent> {
        let previous = self.hat_positions.insert(axis, value).unwrap_or(0);
        if previous == value {
            return Vec::new();
        }
        let released = hat_axis
            .hat_button(previous)
            .map(|button| self.button(button, ButtonEdge::Released));
        let pressed = hat_axis
            .hat_button(value)
            .map(|button| self.button(button, ButtonEdge::Pressed));
        released.into_iter().chain(pressed).collect()
    }

    fn analog_trigger(
        &mut self,
        axis: AbsoluteAxisCode,
        button: GamepadButton,
        raw: i32,
    ) -> Option<RawGamepadEvent> {
        if self.trigger_kind == TriggerKind::Digital {
            return None;
        }
        let pull = self.ranges.get(&axis)?.pull(raw);
        let held = self
            .analog_triggers_held
            .get(&axis)
            .copied()
            .unwrap_or(false);
        let now_held = if held {
            pull > ANALOG_TRIGGER_RELEASE
        } else {
            pull >= ANALOG_TRIGGER_ENGAGE
        };
        self.analog_triggers_held.insert(axis, now_held);
        (now_held != held).then(|| {
            self.button(
                button,
                if now_held {
                    ButtonEdge::Pressed
                } else {
                    ButtonEdge::Released
                },
            )
        })
    }

    pub fn translate(&mut self, event: InputEvent) -> Vec<RawGamepadEvent> {
        match event.destructure() {
            EventSummary::Key(_, key, value @ (0 | 1)) => standard_button(key)
                .map(|button| {
                    let edge = if value == 1 {
                        ButtonEdge::Pressed
                    } else {
                        ButtonEdge::Released
                    };
                    self.button(button, edge)
                })
                .into_iter()
                .collect(),
            EventSummary::AbsoluteAxis(_, axis @ AbsoluteAxisCode::ABS_HAT0X, value) => {
                self.hat(axis, PlaneAxis::Horizontal, value)
            }
            EventSummary::AbsoluteAxis(_, axis @ AbsoluteAxisCode::ABS_HAT0Y, value) => {
                self.hat(axis, PlaneAxis::Vertical, value)
            }
            EventSummary::AbsoluteAxis(_, axis @ AbsoluteAxisCode::ABS_Z, value) => self
                .analog_trigger(axis, GamepadButton::LeftTrigger, value)
                .into_iter()
                .collect(),
            EventSummary::AbsoluteAxis(_, axis @ AbsoluteAxisCode::ABS_RZ, value) => self
                .analog_trigger(axis, GamepadButton::RightTrigger, value)
                .into_iter()
                .collect(),
            EventSummary::AbsoluteAxis(_, axis, value) => {
                self.stick(axis, value).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TriggerKind {
    Digital,
    #[default]
    Analog,
}

#[cfg(test)]
mod tests {
    use evdev::{AbsoluteAxisEvent, KeyEvent};

    use super::*;

    fn dualsense_like_translator() -> GamepadEventTranslator {
        let byte_range = AxisRange {
            minimum: 0,
            maximum: 255,
        };
        GamepadEventTranslator::new(
            31,
            HashMap::from([
                (AbsoluteAxisCode::ABS_X, byte_range),
                (AbsoluteAxisCode::ABS_Y, byte_range),
                (AbsoluteAxisCode::ABS_Z, byte_range),
            ]),
            TriggerKind::Analog,
        )
    }

    fn axis(axis: AbsoluteAxisCode, value: i32) -> InputEvent {
        *AbsoluteAxisEvent::new(axis, value)
    }

    #[test]
    fn the_cross_button_reports_as_south() {
        let events = dualsense_like_translator().translate(*KeyEvent::new(KeyCode::BTN_SOUTH, 1));
        assert_eq!(
            events,
            [RawGamepadEvent::Button {
                slot: 31,
                button: GamepadButton::South,
                edge: ButtonEdge::Pressed,
            }]
        );
    }

    #[test]
    fn pushing_the_left_stick_up_deflects_it_positively() {
        let events = dualsense_like_translator().translate(axis(AbsoluteAxisCode::ABS_Y, 0));
        assert_eq!(
            events,
            [RawGamepadEvent::Stick {
                slot: 31,
                stick: GamepadStick::Left,
                axis: PlaneAxis::Vertical,
                deflection: 1.0,
            }]
        );
    }

    #[test]
    fn a_resting_stick_reads_as_centered() {
        let events = dualsense_like_translator().translate(axis(AbsoluteAxisCode::ABS_X, 130));
        assert!(matches!(
            events.as_slice(),
            [RawGamepadEvent::Stick { deflection, .. }] if *deflection == 0.0
        ));
    }

    #[test]
    fn moving_the_hat_releases_the_previous_dpad_direction() {
        let mut translator = dualsense_like_translator();
        translator.translate(axis(AbsoluteAxisCode::ABS_HAT0X, -1));
        let events = translator.translate(axis(AbsoluteAxisCode::ABS_HAT0X, 1));
        assert_eq!(
            events,
            [
                RawGamepadEvent::Button {
                    slot: 31,
                    button: GamepadButton::DpadLeft,
                    edge: ButtonEdge::Released,
                },
                RawGamepadEvent::Button {
                    slot: 31,
                    button: GamepadButton::DpadRight,
                    edge: ButtonEdge::Pressed,
                },
            ]
        );
    }

    #[test]
    fn an_analog_trigger_pulled_halfway_presses_once() {
        let mut translator = dualsense_like_translator();
        assert_eq!(
            translator
                .translate(axis(AbsoluteAxisCode::ABS_Z, 200))
                .len(),
            1
        );
        assert!(
            translator
                .translate(axis(AbsoluteAxisCode::ABS_Z, 220))
                .is_empty()
        );
        assert_eq!(
            translator.translate(axis(AbsoluteAxisCode::ABS_Z, 10)),
            [RawGamepadEvent::Button {
                slot: 31,
                button: GamepadButton::LeftTrigger,
                edge: ButtonEdge::Released,
            }]
        );
    }
}
