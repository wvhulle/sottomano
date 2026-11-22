mod button_role_actions;
mod connected_controller_indicator;
mod controller_battery_indicator;
pub mod controller_button_labels;
mod controller_discovery_failure;
mod controller_group_rows;
mod controller_link_indicator;
mod diagram_anchor_mapping;
#[cfg(test)]
mod gamepad_group_tests;
mod sqlite_compositor_driving_controllers;
mod steam_input_route;
mod steam_radio_firmware_indicators;
mod usb_cable_indicator;
mod wifi_band_use;
use std::collections::{BTreeMap, BTreeSet};

use iced::{Subscription, Task};
use notulist_client::DictationCommand;
use sottomano_compositor_model::compositor_requests::{
    CompositorRequest, CompositorRequestFailure,
};
use sottomano_desktop_sinks::{
    network_connectivity::bluez_bluetooth_commands::disconnect_bluetooth_address,
    voice_dictation_input::notulist_dictation_commands::send_dictation_command,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    gamepad_controller_input::{
        accelerometer_report_timing::ReportTiming,
        evdev_gamepad_devices::{
            ButtonEdge, ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadTransport,
            PhysicalGamepadLink,
        },
        gamepad_input_machine::{GamepadInput, GamepadStreamSettings, PointerGate},
        gamepad_input_stream::gamepad_inputs,
    },
    network_connectivity::{ethernet_connection::Ethernet, wifi_connection::Wifi},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
};
use sottomano_toml_config::indicator_groups::gamepad_bindings::{
    ButtonRole, GamepadBindingsConfig, GamepadButton,
};

use self::{
    button_role_actions::role_request, controller_discovery_failure::ControllerDiscovery,
    controller_link_indicator::WiredFallback, wifi_band_use::WifiBandUse,
};
use crate::{
    background_work::{compositor_request, logged_failure_task, snapshot_subscription},
    group_indicators::toggle_control::SwitchPosition,
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::indicator_group_content::{
        IndicatorGroupExpansion, IndicatorGroupExpansionMessage, KeyedExpansions,
    },
};

#[derive(Debug)]
pub enum GamepadMessage {
    Input(GamepadInput),
    DrivingControllersLoaded(BTreeSet<String>),
    KernelErrors(Vec<DeviceKernelError>),
    Wifi(Box<SnapshotDelivery<Wifi>>),
    Ethernet(Box<SnapshotDelivery<Ethernet>>),
}

#[derive(Debug, Clone, Copy)]
pub enum GamepadInteraction {
    DisconnectBluetooth {
        slot: usize,
    },
    ToggleCompositorDriving {
        slot: usize,
    },
    Expansion {
        slot: usize,
        expansion: IndicatorGroupExpansionMessage,
    },
    ControllerDiscoveryExpansion(IndicatorGroupExpansionMessage),
}

#[derive(Debug, Clone)]
pub enum Action {
    Idle,
    Compositor(CompositorRequest),
    Dictation(DictationCommand),
    DisconnectBluetooth { hardware_address: String },
    SaveDrivingControllers(BTreeSet<String>),
}

impl Action {
    pub fn into_task<M: From<CompositorRequestFailure> + Send + 'static>(self) -> Task<M> {
        match self {
            Action::Idle => Task::none(),
            Action::Compositor(request) => compositor_request(request),
            Action::Dictation(command) => logged_failure_task(
                format!("notulist {command:?}"),
                send_dictation_command(command),
            ),
            Action::DisconnectBluetooth { hardware_address } => {
                logged_failure_task("disconnect Bluetooth controller".to_owned(), async move {
                    disconnect_bluetooth_address(hardware_address).await
                })
            }
            Action::SaveDrivingControllers(driving_controllers) => logged_failure_task(
                "save gamepad compositor control".to_owned(),
                sqlite_compositor_driving_controllers::save(driving_controllers),
            ),
        }
    }
}

#[derive(Default)]
pub struct GamepadGroup {
    controllers: Vec<ConnectedGamepad>,
    expansions: KeyedExpansions<usize>,
    report_timings: BTreeMap<usize, ReportTiming>,
    kernel_errors: Vec<DeviceKernelError>,
    wifi_band_use: WifiBandUse,
    wired_fallback: WiredFallback,
    compositor_driving_controllers: BTreeSet<String>,
    controller_discovery: ControllerDiscovery,
    controller_discovery_expansion: IndicatorGroupExpansion,
}

fn controller_custody_key(link: &PhysicalGamepadLink) -> &str {
    match &link.identity {
        ControllerIdentity::Addressed {
            hardware_address, ..
        } => hardware_address,
        ControllerIdentity::Unaddressed => &link.kernel_device,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerCustody {
    Compositor,
    HandedOver,
}

impl From<ControllerCustody> for SwitchPosition {
    fn from(controller_custody: ControllerCustody) -> Self {
        match controller_custody {
            ControllerCustody::Compositor => SwitchPosition::On,
            ControllerCustody::HandedOver => SwitchPosition::Off,
        }
    }
}

impl GamepadGroup {
    pub fn collapse(&mut self) {
        self.expansions.collapse();
        self.controller_discovery_expansion.collapse();
    }

    pub fn load() -> Task<GamepadMessage> {
        Task::perform(
            sqlite_compositor_driving_controllers::load(),
            GamepadMessage::DrivingControllersLoaded,
        )
    }

    fn controller_custody(&self, slot: usize) -> ControllerCustody {
        match self.physical_link(slot) {
            Some(link)
                if self
                    .compositor_driving_controllers
                    .contains(controller_custody_key(link)) =>
            {
                ControllerCustody::Compositor
            }
            _ => ControllerCustody::HandedOver,
        }
    }

    fn compositor_driving_slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.controllers
            .iter()
            .map(|controller| controller.slot)
            .filter(|slot| self.controller_custody(*slot) == ControllerCustody::Compositor)
    }

    #[must_use]
    pub fn stream_settings(&self, bindings: &GamepadBindingsConfig) -> GamepadStreamSettings {
        GamepadStreamSettings {
            steering: bindings.pointer_steering(),
            gate: PointerGate::driving_slots(self.compositor_driving_slots()),
        }
    }

    pub fn subscription(&self, bindings: &GamepadBindingsConfig) -> Subscription<GamepadMessage> {
        Subscription::batch([
            Subscription::run_with(self.stream_settings(bindings), |settings| {
                gamepad_inputs(settings.clone())
            })
            .map(GamepadMessage::Input),
            Subscription::run(device_kernel_error_windows).map(GamepadMessage::KernelErrors),
        ])
    }

    pub fn network_subscription(
        &self,
        buses: &DesktopBuses,
        demand: GroupSourceDemand,
    ) -> Subscription<GamepadMessage> {
        let session_expansion = self.expansions.combined_state();
        let demand = demand.expanded_when(session_expansion);
        Subscription::batch([
            demand.while_indicators_shown(
                snapshot_subscription::<Wifi>(buses.clone())
                    .map(|delivery| GamepadMessage::Wifi(Box::new(delivery))),
            ),
            demand.while_indicators_shown(
                snapshot_subscription::<Ethernet>(buses.clone())
                    .map(|delivery| GamepadMessage::Ethernet(Box::new(delivery))),
            ),
        ])
    }

    pub fn update(&mut self, message: GamepadMessage, bindings: &GamepadBindingsConfig) -> Action {
        match message {
            GamepadMessage::Input(GamepadInput::Roster(controllers)) => {
                self.controller_discovery = ControllerDiscovery::Watching;
                self.receive_roster(controllers);
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::DiscoveryFailed(discovery_failure)) => {
                self.controller_discovery = ControllerDiscovery::Failed(discovery_failure);
                Action::Idle
            }
            GamepadMessage::DrivingControllersLoaded(driving_controllers) => {
                self.compositor_driving_controllers = driving_controllers;
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::ReportTiming { slot, timing }) => {
                self.report_timings.insert(slot, timing);
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::Button { slot, button, edge }) => {
                self.receive_button(slot, button, edge, bindings)
            }
            GamepadMessage::Input(GamepadInput::StickFlick {
                slot,
                stick,
                direction,
            }) => bindings
                .navigation
                .flick_action(stick, direction)
                .filter(|_| self.controller_custody(slot) == ControllerCustody::Compositor)
                .map_or(Action::Idle, |action| {
                    Action::Compositor(CompositorRequest::from(action))
                }),
            GamepadMessage::KernelErrors(kernel_errors) => {
                self.kernel_errors = kernel_errors;
                Action::Idle
            }
            GamepadMessage::Wifi(delivery) => {
                self.receive_wifi_delivery(*delivery);
                Action::Idle
            }
            GamepadMessage::Ethernet(delivery) => {
                self.receive_ethernet_delivery(*delivery);
                Action::Idle
            }
        }
    }

    pub fn interact(&mut self, interaction: GamepadInteraction) -> Action {
        match interaction {
            GamepadInteraction::DisconnectBluetooth { slot } => self
                .bluetooth_address(slot)
                .map_or(Action::Idle, |hardware_address| {
                    Action::DisconnectBluetooth {
                        hardware_address: hardware_address.to_owned(),
                    }
                }),
            GamepadInteraction::ToggleCompositorDriving { slot } => {
                let Some(key) = self.physical_link(slot).map(controller_custody_key) else {
                    return Action::Idle;
                };
                let key = key.to_owned();
                if !self.compositor_driving_controllers.remove(&key) {
                    self.compositor_driving_controllers.insert(key);
                }
                Action::SaveDrivingControllers(self.compositor_driving_controllers.clone())
            }
            GamepadInteraction::Expansion { slot, expansion } => {
                self.expansions.update(&slot, expansion);
                Action::Idle
            }
            GamepadInteraction::ControllerDiscoveryExpansion(expansion) => {
                self.controller_discovery_expansion.update(expansion);
                Action::Idle
            }
        }
    }

    fn receive_roster(&mut self, controllers: Vec<ConnectedGamepad>) {
        let connected = |slot: &usize| controllers.iter().any(|gamepad| gamepad.slot == *slot);
        self.expansions
            .track(controllers.iter().map(|gamepad| gamepad.slot));
        self.report_timings.retain(|slot, _| connected(slot));
        self.controllers = controllers;
    }

    fn receive_button(
        &self,
        slot: usize,
        button: GamepadButton,
        edge: ButtonEdge,
        bindings: &GamepadBindingsConfig,
    ) -> Action {
        let releasing_push_to_talk = edge == ButtonEdge::Released
            && bindings.claimed_role(button) == Some(ButtonRole::PushToTalk);
        if !releasing_push_to_talk && self.controller_custody(slot) != ControllerCustody::Compositor
        {
            return Action::Idle;
        }
        bindings
            .claimed_role(button)
            .and_then(|role| role_request(role, edge))
            .unwrap_or(Action::Idle)
    }

    fn physical_controllers(
        &self,
    ) -> impl Iterator<Item = (&ConnectedGamepad, &PhysicalGamepadLink)> {
        self.controllers
            .iter()
            .filter_map(|controller| match &controller.origin {
                GamepadOrigin::Physical(link) => Some((controller, link)),
                GamepadOrigin::Virtual => None,
            })
    }

    fn physical_link(&self, slot: usize) -> Option<&PhysicalGamepadLink> {
        self.physical_controllers()
            .find(|(controller, _)| controller.slot == slot)
            .map(|(_, link)| link)
    }

    #[must_use]
    pub fn report_timing_for_address(&self, hardware_address: &str) -> Option<ReportTiming> {
        self.controllers
            .iter()
            .map(|controller| controller.slot)
            .find(|slot| {
                self.bluetooth_address(*slot)
                    .is_some_and(|address| address.eq_ignore_ascii_case(hardware_address))
            })
            .and_then(|slot| self.report_timings.get(&slot).copied())
    }

    fn bluetooth_address(&self, slot: usize) -> Option<&str> {
        let link = self.physical_link(slot)?;
        match (&link.transport, &link.identity) {
            (
                GamepadTransport::Bluetooth { .. },
                ControllerIdentity::Addressed {
                    hardware_address, ..
                },
            ) => Some(hardware_address),
            _ => None,
        }
    }
}
