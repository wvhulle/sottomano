use std::{
    collections::HashMap,
    future::ready,
    path::{Path, PathBuf},
    time::Duration,
};

use evdev::{AbsoluteAxisCode, BusType, Device, EventSummary, InputEvent, KeyCode};
use futures::{Stream, StreamExt, stream};
use sottomano_toml_config::widget_groups::gamepad_bindings::{GamepadButton, GamepadStick};
use tokio::time::sleep;

use super::{
    accelerometer_report_timing::{ReportTiming, accelerometer_node_beside, report_timings},
    bluez_device_aliases::bluetooth_aliases,
    player_indicator_leds::PlayerIndicator,
};
use crate::{
    delivery_cadence_drivers::PeriodicPolling,
    stream_operators::{StreamOperators, read_once_then_per_burst},
    system_protocol_clients::{
        inotify_directory_changes::{self, WatchTarget},
        sysfs_attribute_reads,
    },
};

const INPUT_DEVICE_DIRECTORY: &str = "/dev/input";
const PERMISSION_SETTLE_DELAY: Duration = Duration::from_millis(400);
const HID_DEVICE_DIRECTORY: &str = "/sys/bus/hid/devices";
const USB_HID_DEVICE_PREFIX: &str = "0003:";
const ROSTER_REFRESH: Duration = Duration::from_secs(10);
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

impl From<usize> for GamepadPower {
    fn from(slot: usize) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsbCableState {
    Absent,
    IgnoredByKernel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadTransport {
    Bluetooth { usb_cable: UsbCableState },
    Usb,
    Other,
}

impl GamepadTransport {
    fn refreshed(self, identity: &ControllerIdentity) -> Self {
        match (self, identity) {
            (
                GamepadTransport::Bluetooth { .. },
                ControllerIdentity::Addressed {
                    hardware_address, ..
                },
            ) => GamepadTransport::Bluetooth {
                usb_cable: if has_unbound_usb_cable(hardware_address) {
                    UsbCableState::IgnoredByKernel
                } else {
                    UsbCableState::Absent
                },
            },
            (transport, _) => transport,
        }
    }
}

impl From<BusType> for GamepadTransport {
    fn from(bus: BusType) -> Self {
        match bus {
            BusType::BUS_BLUETOOTH => GamepadTransport::Bluetooth {
                usb_cable: UsbCableState::Absent,
            },
            BusType::BUS_USB => GamepadTransport::Usb,
            _ => GamepadTransport::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControllerIdentity {
    Addressed {
        hardware_address: String,
        alias: String,
    },
    Unaddressed,
}

impl ControllerIdentity {
    fn refreshed(&self, bluetooth_aliases: &HashMap<String, String>) -> Self {
        match self {
            ControllerIdentity::Addressed {
                hardware_address,
                alias,
            } => ControllerIdentity::Addressed {
                hardware_address: hardware_address.clone(),
                alias: bluetooth_aliases
                    .get(&hardware_address.to_lowercase())
                    .unwrap_or(alias)
                    .clone(),
            },
            ControllerIdentity::Unaddressed => ControllerIdentity::Unaddressed,
        }
    }
}

impl From<&Device> for ControllerIdentity {
    fn from(device: &Device) -> Self {
        match device.unique_name().filter(|unique| !unique.is_empty()) {
            Some(hardware_address) => ControllerIdentity::Addressed {
                hardware_address: hardware_address.to_owned(),
                alias: device.name().unwrap_or(hardware_address).to_owned(),
            },
            None => ControllerIdentity::Unaddressed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalGamepadLink {
    pub kernel_device: String,
    pub transport: GamepadTransport,
    pub identity: ControllerIdentity,
    pub player_indicator: PlayerIndicator,
}

impl PhysicalGamepadLink {
    fn refreshed(&self, slot: usize, bluetooth_aliases: &HashMap<String, String>) -> Self {
        let identity = self.identity.refreshed(bluetooth_aliases);
        PhysicalGamepadLink {
            kernel_device: self.kernel_device.clone(),
            transport: self.transport.refreshed(&identity),
            identity,
            player_indicator: PlayerIndicator::from(slot),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GamepadOrigin {
    Physical(PhysicalGamepadLink),
    Virtual,
}

impl GamepadOrigin {
    fn of_input_node(slot: usize, device: &Device) -> Self {
        kernel_device_of_input_node(slot).map_or(GamepadOrigin::Virtual, |kernel_device| {
            GamepadOrigin::Physical(PhysicalGamepadLink {
                kernel_device,
                transport: GamepadTransport::from(device.input_id().bus_type()),
                identity: ControllerIdentity::from(device),
                player_indicator: PlayerIndicator::from(slot),
            })
        })
    }

    fn refreshed(&self, slot: usize, bluetooth_aliases: &HashMap<String, String>) -> Self {
        match self {
            GamepadOrigin::Physical(link) => {
                GamepadOrigin::Physical(link.refreshed(slot, bluetooth_aliases))
            }
            GamepadOrigin::Virtual => GamepadOrigin::Virtual,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedGamepad {
    pub slot: usize,
    pub name: String,
    pub vendor_id: u16,
    pub power: GamepadPower,
    pub origin: GamepadOrigin,
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
    ReportTiming {
        slot: usize,
        timing: ReportTiming,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputDeviceKind {
    Gamepad,
    OtherInput,
}

impl From<&Device> for InputDeviceKind {
    fn from(device: &Device) -> Self {
        let has_face_buttons = device
            .supported_keys()
            .is_some_and(|keys| keys.contains(KeyCode::BTN_SOUTH));
        let has_left_stick = device.supported_absolute_axes().is_some_and(|axes| {
            axes.contains(AbsoluteAxisCode::ABS_X) && axes.contains(AbsoluteAxisCode::ABS_Y)
        });
        if has_face_buttons && has_left_stick {
            InputDeviceKind::Gamepad
        } else {
            InputDeviceKind::OtherInput
        }
    }
}

fn input_node_number(path: &Path) -> Option<usize> {
    path.file_name()?
        .to_str()?
        .strip_prefix("event")?
        .parse()
        .ok()
}

fn kernel_device_of_input_node(slot: usize) -> Option<String> {
    let parent = Path::new(&format!("/sys/class/input/event{slot}/device/device"))
        .canonicalize()
        .ok()?;
    let subsystem = parent.join("subsystem").canonicalize().ok()?;
    Some(format!(
        "+{}:{}",
        subsystem.file_name()?.to_str()?,
        parent.file_name()?.to_str()?
    ))
}

fn has_unbound_usb_cable(hardware_address: &str) -> bool {
    sysfs_attribute_reads::list_dir(HID_DEVICE_DIRECTORY)
        .into_iter()
        .filter(|hid_device| {
            hid_device
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(USB_HID_DEVICE_PREFIX))
        })
        .any(|hid_device| {
            !hid_device.join("driver").exists()
                && sysfs_attribute_reads::uevent_field(&hid_device.join("uevent"), "HID_UNIQ")
                    .is_some_and(|unique| unique.eq_ignore_ascii_case(hardware_address))
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredGamepad {
    slot: usize,
    name: String,
    vendor_id: u16,
    origin: GamepadOrigin,
}

impl DiscoveredGamepad {
    fn input_node(&self) -> PathBuf {
        Path::new(INPUT_DEVICE_DIRECTORY).join(format!("event{}", self.slot))
    }

    fn connected(&self, bluetooth_aliases: &HashMap<String, String>) -> ConnectedGamepad {
        ConnectedGamepad {
            slot: self.slot,
            name: self.name.clone(),
            vendor_id: self.vendor_id,
            power: GamepadPower::from(self.slot),
            origin: self.origin.refreshed(self.slot, bluetooth_aliases),
        }
    }
}

fn discovered_gamepads() -> Vec<DiscoveredGamepad> {
    let mut gamepads: Vec<DiscoveredGamepad> = evdev::enumerate()
        .filter(|(_, device)| InputDeviceKind::from(device) == InputDeviceKind::Gamepad)
        .filter_map(|(path, device): (PathBuf, Device)| {
            let slot = input_node_number(&path)?;
            Some(DiscoveredGamepad {
                slot,
                name: device.name().unwrap_or("Game controller").to_owned(),
                vendor_id: device.input_id().vendor(),
                origin: GamepadOrigin::of_input_node(slot, &device),
            })
        })
        .collect();
    gamepads.sort_by_key(|gamepad| (gamepad.origin == GamepadOrigin::Virtual, gamepad.slot));
    gamepads
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

fn roster_with_refresh(
    gamepads: Vec<DiscoveredGamepad>,
) -> impl Stream<Item = RawGamepadEvent> + Send {
    stream::once(async {
        zbus::Connection::system()
            .await
            .inspect_err(|e| tracing::warn!("gamepad: no system bus for Bluetooth aliases: {e}"))
            .ok()
    })
    .flat_map(move |system_bus| {
        let gamepads = gamepads.clone();
        PeriodicPolling::every(ROSTER_REFRESH)
            .poll_ticks()
            .then(move |_| {
                let system_bus = system_bus.clone();
                let gamepads = gamepads.clone();
                async move {
                    let aliases = match &system_bus {
                        Some(connection) => bluetooth_aliases(connection).await,
                        None => HashMap::new(),
                    };
                    RawGamepadEvent::Roster(
                        gamepads
                            .iter()
                            .map(|gamepad| gamepad.connected(&aliases))
                            .collect(),
                    )
                }
            })
    })
}

fn connected_gamepad_events(
    gamepads: Vec<DiscoveredGamepad>,
) -> impl Stream<Item = RawGamepadEvent> + Send {
    let physical_gamepads = gamepads
        .iter()
        .filter(|gamepad| gamepad.origin != GamepadOrigin::Virtual);
    let button_events = physical_gamepads
        .clone()
        .map(|gamepad| gamepad_events(gamepad).boxed());
    let report_rate_events = physical_gamepads.filter_map(|gamepad| {
        accelerometer_node_beside(gamepad.slot)
            .map(|accelerometer| report_timings(gamepad.slot, &accelerometer).boxed())
    });
    let device_events = stream::select_all(button_events.chain(report_rate_events));
    stream::select(roster_with_refresh(gamepads), device_events)
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
