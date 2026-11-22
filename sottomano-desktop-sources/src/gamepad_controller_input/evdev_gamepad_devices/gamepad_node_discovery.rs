use std::{
    collections::HashMap,
    future::ready,
    path::{Path, PathBuf},
    time::Duration,
};

use evdev::{AbsoluteAxisCode, Device, EventStream, KeyCode};
use futures::{Stream, StreamExt, stream};
use futures_rx::RxExt;
use tokio::time::sleep;

use super::{
    ConnectedGamepad, GamepadOrigin, GamepadPower, RawGamepadEvent,
    gamepad_event_translation::GamepadEventTranslator, is_steam_virtual_pad,
};
use crate::{
    delivery_cadence_drivers::{ChangeTriggerStreams, PeriodicPolling},
    gamepad_controller_input::{
        accelerometer_report_timing::{accelerometer_node_beside, report_timings},
        bluez_device_aliases::bluetooth_aliases,
        steam_dongle_hidraw::{dongle_button_changes, dongle_interfaces, dongle_samples},
        steam_input_pairing::{DeviceSignal, SteamInputPairing},
    },
    steam_client_files::{
        steam_controller_facts::steam_controller_facts, steam_root_location::SteamRoot,
    },
    system_protocol_clients::inotify_directory_changes::{self, WatchTarget},
};

const INPUT_DEVICE_DIRECTORY: &str = "/dev/input";
const PERMISSION_SETTLE_DELAY: Duration = Duration::from_millis(400);
const ROSTER_REFRESH: Duration = Duration::from_secs(10);

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
    let node_number_text = path.file_name()?.to_str()?.strip_prefix("event")?;
    node_number_text
        .parse()
        .inspect_err(|parse_failure| {
            tracing::debug!(
                "gamepad: input node {} has no number: {parse_failure}",
                path.display()
            );
        })
        .ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredGamepad {
    slot: usize,
    name: String,
    vendor_id: u16,
    product_id: u16,
    origin: GamepadOrigin,
}

impl DiscoveredGamepad {
    fn is_steam_virtual_pad(&self) -> bool {
        is_steam_virtual_pad(self.vendor_id, self.product_id, &self.origin)
    }

    fn input_node(&self) -> PathBuf {
        Path::new(INPUT_DEVICE_DIRECTORY).join(format!("event{}", self.slot))
    }

    fn connected(&self, bluetooth_aliases: &HashMap<String, String>) -> ConnectedGamepad {
        ConnectedGamepad {
            slot: self.slot,
            name: self.name.clone(),
            vendor_id: self.vendor_id,
            product_id: self.product_id,
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
                product_id: device.input_id().product(),
                origin: GamepadOrigin::probe_input_node(slot, &device),
            })
        })
        .collect();
    gamepads.sort_by_key(|gamepad| (gamepad.origin == GamepadOrigin::Virtual, gamepad.slot));
    gamepads
}

fn gamepad_events(
    gamepad: &DiscoveredGamepad,
) -> impl Stream<Item = RawGamepadEvent> + Send + use<> {
    let opened_device = Device::open(gamepad.input_node())
        .and_then(|device| {
            let translator = GamepadEventTranslator::probe_device_axes(gamepad.slot, &device);
            Ok(OpenedGamepad {
                translator,
                events: device.into_event_stream()?,
            })
        })
        .inspect_err(|open_failure| {
            tracing::warn!("gamepad: cannot read {}: {open_failure}", gamepad.name);
        })
        .ok();
    let gamepad_name = gamepad.name.clone();
    stream::iter(opened_device).flat_map(move |opened_gamepad| {
        let gamepad_name = gamepad_name.clone();
        opened_gamepad
            .events
            .scan((), move |(), event| {
                ready(
                    event
                        .inspect_err(|read_failure| {
                            tracing::debug!(
                                "gamepad: events of {gamepad_name} ended: {read_failure}"
                            );
                        })
                        .ok(),
                )
            })
            .scan(opened_gamepad.translator, |translator, event| {
                ready(Some(stream::iter(translator.translate(event))))
            })
            .flatten()
    })
}

struct OpenedGamepad {
    translator: GamepadEventTranslator,
    events: EventStream,
}

fn roster_with_refresh(
    gamepads: Vec<DiscoveredGamepad>,
) -> impl Stream<Item = RawGamepadEvent> + Send {
    stream::once(async {
        zbus::Connection::system()
            .await
            .inspect_err(|error| {
                tracing::warn!("gamepad: no system bus for Bluetooth aliases: {error}");
            })
            .ok()
    })
    .flat_map(move |system_bus| {
        let gamepads = gamepads.clone();
        PeriodicPolling::every(ROSTER_REFRESH)
            .poll_ticks()
            .map(move |_| {
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
            .buffered(1)
    })
}

fn connected_gamepad_events(
    gamepads: Vec<DiscoveredGamepad>,
) -> impl Stream<Item = RawGamepadEvent> + Send {
    let physical_gamepads = gamepads
        .iter()
        .filter(|gamepad| gamepad.origin != GamepadOrigin::Virtual);
    let steam_virtual_pads = gamepads
        .iter()
        .filter(|gamepad| gamepad.is_steam_virtual_pad());
    let steam_virtual_pad_count = steam_virtual_pads.clone().count();
    let button_events = physical_gamepads
        .clone()
        .chain(steam_virtual_pads)
        .map(|gamepad| gamepad_events(gamepad).boxed());
    let report_rate_events = physical_gamepads.filter_map(|gamepad| {
        accelerometer_node_beside(gamepad.slot)
            .map(|accelerometer| report_timings(gamepad.slot, &accelerometer).boxed())
    });
    let watched_dongle_interfaces = if steam_virtual_pad_count > 0 {
        dongle_interfaces()
    } else {
        Vec::new()
    };
    let correlated_interfaces = if steam_virtual_pad_count > 1 {
        watched_dongle_interfaces.clone()
    } else {
        Vec::new()
    };
    let battery_signals = watched_dongle_interfaces.into_iter().map(|interface| {
        dongle_samples(interface)
            .map(|(interface, sample)| DeviceSignal::Dongle(interface, sample))
            .boxed()
    });
    let button_change_signals = correlated_interfaces.into_iter().map(|interface| {
        dongle_button_changes(interface)
            .map(|(interface, changed_at)| {
                DeviceSignal::DongleButtonsChanged(interface, changed_at)
            })
            .boxed()
    });
    let steam_root = if steam_virtual_pad_count > 0 {
        SteamRoot::located()
    } else {
        None
    };
    let steam_fact_signals = steam_root.map(|steam_root| {
        steam_controller_facts(&steam_root)
            .map(|facts| DeviceSignal::SteamFacts(Box::new(facts)))
            .boxed()
    });
    let dongle_signals = battery_signals
        .chain(button_change_signals)
        .chain(steam_fact_signals);
    let device_events = stream::select_all(button_events.chain(report_rate_events));
    let raw_signals =
        stream::select(roster_with_refresh(gamepads), device_events).map(DeviceSignal::Raw);
    stream::select(raw_signals, stream::select_all(dongle_signals))
        .scan(SteamInputPairing::default(), |pairing, signal| {
            ready(Some(stream::iter(pairing.absorb(signal))))
        })
        .flatten()
}

pub fn raw_gamepad_events() -> impl Stream<Item = RawGamepadEvent> + Send {
    let device_changes = inotify_directory_changes::changes(vec![WatchTarget::NamesIn(
        PathBuf::from(INPUT_DEVICE_DIRECTORY),
    )]);
    device_changes
        .triggered_rereads(|| async {
            sleep(PERMISSION_SETTLE_DELAY).await;
            Ok(discovered_gamepads())
        })
        .scan(
            None,
            |known_gamepads: &mut Option<Vec<DiscoveredGamepad>>, discovery| {
                ready(Some(match discovery {
                    Ok(gamepads) if known_gamepads.as_ref() == Some(&gamepads) => None,
                    Ok(gamepads) => {
                        *known_gamepads = Some(gamepads.clone());
                        Some(GamepadDiscovery {
                            failure_events: Vec::new(),
                            gamepads,
                        })
                    }
                    Err(watch_failure) => Some(GamepadDiscovery {
                        failure_events: vec![RawGamepadEvent::DiscoveryFailed(watch_failure)],
                        gamepads: known_gamepads.clone().unwrap_or_default(),
                    }),
                }))
            },
        )
        .filter_map(ready)
        .switch_map(|discovery| {
            stream::iter(discovery.failure_events)
                .chain(connected_gamepad_events(discovery.gamepads))
        })
}

struct GamepadDiscovery {
    failure_events: Vec<RawGamepadEvent>,
    gamepads: Vec<DiscoveredGamepad>,
}
