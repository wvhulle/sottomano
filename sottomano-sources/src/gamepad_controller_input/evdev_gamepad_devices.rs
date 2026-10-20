use std::{
    collections::HashMap,
    future::ready,
    path::{Path, PathBuf},
    time::Duration,
};

use evdev::{AbsoluteAxisCode, Device, EventSummary, InputEvent, KeyCode};
use futures::{Stream, StreamExt, stream};
use sottomano_config::widget_groups::gamepad_bindings::{GamepadButton, GamepadStick};
use tokio::time::sleep;

use crate::{
    stream_operators::{StreamOperators, read_once_then_per_burst, ticked},
    system_protocol_clients::{
        inotify_directory_changes::{self, WatchTarget},
        sysfs_attribute_reads,
    },
};

const INPUT_DEVICE_DIRECTORY: &str = "/dev/input";
const PERMISSION_SETTLE_DELAY: Duration = Duration::from_millis(400);
const BATTERY_REFRESH: Duration = Duration::from_mins(1);
const STICK_DEADZONE: f32 = 0.12;
const ANALOG_TRIGGER_ENGAGE: f32 = 0.5;
const ANALOG_TRIGGER_RELEASE: f32 = 0.3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadPower {
    NoBattery,
    Discharging(u8),
    Charging(u8),
    Full,
}

impl GamepadPower {
    fn of_input_node(slot: usize) -> Self {
        let supplies = sysfs_attribute_reads::list_dir(format!(
            "/sys/class/input/event{slot}/device/device/power_supply"
        ));
        let Some(supply) = supplies.first() else {
            return GamepadPower::NoBattery;
        };
        let capacity = sysfs_attribute_reads::read_i64(supply.join("capacity"))
            .map_or(0, |percent| percent.clamp(0, 100) as u8);
        match sysfs_attribute_reads::read_trimmed(supply.join("status")).as_deref() {
            Some("Charging") => GamepadPower::Charging(capacity),
            Some("Full") => GamepadPower::Full,
            _ => GamepadPower::Discharging(capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedGamepad {
    pub slot: usize,
    pub name: String,
    pub vendor_id: u16,
    pub power: GamepadPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonEdge {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RawGamepadEvent {
    Roster(Vec<ConnectedGamepad>),
    Button {
        slot: usize,
        button: GamepadButton,
        edge: ButtonEdge,
    },
    Stick {
        slot: usize,
        stick: GamepadStick,
        axis: StickAxis,
        deflection: f32,
    },
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HatAxis {
    Horizontal,
    Vertical,
}

impl HatAxis {
    const fn button(self, value: i32) -> Option<GamepadButton> {
        match (self, value.signum()) {
            (HatAxis::Horizontal, -1) => Some(GamepadButton::DpadLeft),
            (HatAxis::Horizontal, 1) => Some(GamepadButton::DpadRight),
            (HatAxis::Vertical, -1) => Some(GamepadButton::DpadUp),
            (HatAxis::Vertical, 1) => Some(GamepadButton::DpadDown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GamepadEventTranslator {
    slot: usize,
    ranges: HashMap<AbsoluteAxisCode, AxisRange>,
    digital_triggers: bool,
    hat_positions: HashMap<AbsoluteAxisCode, i32>,
    analog_triggers_held: HashMap<AbsoluteAxisCode, bool>,
}

impl GamepadEventTranslator {
    #[must_use]
    pub fn new(
        slot: usize,
        ranges: HashMap<AbsoluteAxisCode, AxisRange>,
        digital_triggers: bool,
    ) -> Self {
        Self {
            slot,
            ranges,
            digital_triggers,
            ..Self::default()
        }
    }

    fn of_device(slot: usize, device: &Device) -> Self {
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
        let digital_triggers = device
            .supported_keys()
            .is_some_and(|keys| keys.contains(KeyCode::BTN_TL2));
        Self::new(slot, ranges, digital_triggers)
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
            AbsoluteAxisCode::ABS_X => (GamepadStick::Left, StickAxis::Horizontal, 1.0),
            AbsoluteAxisCode::ABS_Y => (GamepadStick::Left, StickAxis::Vertical, -1.0),
            AbsoluteAxisCode::ABS_RX => (GamepadStick::Right, StickAxis::Horizontal, 1.0),
            AbsoluteAxisCode::ABS_RY => (GamepadStick::Right, StickAxis::Vertical, -1.0),
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
        hat_axis: HatAxis,
        value: i32,
    ) -> Vec<RawGamepadEvent> {
        let previous = self.hat_positions.insert(axis, value).unwrap_or(0);
        if previous == value {
            return Vec::new();
        }
        let released = hat_axis
            .button(previous)
            .map(|button| self.button(button, ButtonEdge::Released));
        let pressed = hat_axis
            .button(value)
            .map(|button| self.button(button, ButtonEdge::Pressed));
        released.into_iter().chain(pressed).collect()
    }

    fn analog_trigger(
        &mut self,
        axis: AbsoluteAxisCode,
        button: GamepadButton,
        raw: i32,
    ) -> Option<RawGamepadEvent> {
        if self.digital_triggers {
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
                self.hat(axis, HatAxis::Horizontal, value)
            }
            EventSummary::AbsoluteAxis(_, axis @ AbsoluteAxisCode::ABS_HAT0Y, value) => {
                self.hat(axis, HatAxis::Vertical, value)
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

fn is_gamepad(device: &Device) -> bool {
    let has_face_buttons = device
        .supported_keys()
        .is_some_and(|keys| keys.contains(KeyCode::BTN_SOUTH));
    let has_left_stick = device.supported_absolute_axes().is_some_and(|axes| {
        axes.contains(AbsoluteAxisCode::ABS_X) && axes.contains(AbsoluteAxisCode::ABS_Y)
    });
    has_face_buttons && has_left_stick
}

fn input_node_number(path: &Path) -> Option<usize> {
    path.file_name()?
        .to_str()?
        .strip_prefix("event")?
        .parse()
        .ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredGamepad {
    slot: usize,
    name: String,
    vendor_id: u16,
}

impl DiscoveredGamepad {
    fn input_node(&self) -> PathBuf {
        Path::new(INPUT_DEVICE_DIRECTORY).join(format!("event{}", self.slot))
    }

    fn connected(&self) -> ConnectedGamepad {
        ConnectedGamepad {
            slot: self.slot,
            name: self.name.clone(),
            vendor_id: self.vendor_id,
            power: GamepadPower::of_input_node(self.slot),
        }
    }
}

fn discovered_gamepads() -> Vec<DiscoveredGamepad> {
    evdev::enumerate()
        .filter(|(_, device)| is_gamepad(device))
        .filter_map(|(path, device): (PathBuf, Device)| {
            Some(DiscoveredGamepad {
                slot: input_node_number(&path)?,
                name: device.name().unwrap_or("Game controller").to_owned(),
                vendor_id: device.input_id().vendor(),
            })
        })
        .collect()
}

fn gamepad_events(
    gamepad: &DiscoveredGamepad,
) -> impl Stream<Item = RawGamepadEvent> + Send + use<> {
    let opened = Device::open(gamepad.input_node()).and_then(|device| {
        let translator = GamepadEventTranslator::of_device(gamepad.slot, &device);
        Ok((translator, device.into_event_stream()?))
    });
    match opened {
        Ok((translator, events)) => events
            .take_while(|event| ready(event.is_ok()))
            .filter_map(|event| ready(event.ok()))
            .scan(translator, |translator, event| {
                ready(Some(stream::iter(translator.translate(event))))
            })
            .flatten()
            .left_stream(),
        Err(e) => {
            tracing::warn!("gamepad: cannot read {}: {e}", gamepad.name);
            stream::empty().right_stream()
        }
    }
}

fn roster_with_battery_refresh(
    gamepads: Vec<DiscoveredGamepad>,
) -> impl Stream<Item = RawGamepadEvent> + Send {
    ticked(BATTERY_REFRESH, move || {
        ready(RawGamepadEvent::Roster(
            gamepads.iter().map(DiscoveredGamepad::connected).collect(),
        ))
    })
}

fn connected_gamepad_events(
    gamepads: Vec<DiscoveredGamepad>,
) -> impl Stream<Item = RawGamepadEvent> + Send {
    let device_events = stream::select_all(
        gamepads
            .iter()
            .map(|gamepad| gamepad_events(gamepad).boxed()),
    );
    stream::select(roster_with_battery_refresh(gamepads), device_events)
}

pub fn raw_gamepad_events() -> impl Stream<Item = RawGamepadEvent> + Send {
    let device_changes = inotify_directory_changes::changes(vec![WatchTarget::NamesIn(
        PathBuf::from(INPUT_DEVICE_DIRECTORY),
    )]);
    read_once_then_per_burst(device_changes, || async {
        sleep(PERMISSION_SETTLE_DELAY).await;
        discovered_gamepads()
    })
    .distinct_until_changed()
    .switch_map(connected_gamepad_events)
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
            false,
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
                axis: StickAxis::Vertical,
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
