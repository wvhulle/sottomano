mod connected_controller_reading;
mod controller_battery_reading;
pub mod controller_button_labels;
mod controller_link_reading;
mod diagram_anchor_mapping;
mod sqlite_compositor_driving_controllers;
mod usb_cable_reading;

use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
};

use chrono::Locale;
use iced::{Subscription, Task};
use notulist_client::DictationCommand;
use sottomano_desktop_sinks::{
    compositor_window_requests::compositor_requests::{
        CompositorRequest, CompositorRequestFailure,
    },
    network_connectivity::bluez_bluetooth_commands::disconnect_bluetooth_address,
    voice_dictation_input::notulist_dictation_commands::send_logged_dictation_command,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    delivery_cadence_drivers::ChangeWatchFailure,
    gamepad_controller_input::{
        accelerometer_report_timing::ReportTiming,
        evdev_gamepad_devices::{
            ButtonEdge, ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadPower,
            GamepadTransport, PhysicalGamepadLink, UsbCableState,
        },
        gamepad_input_machine::{GamepadInput, GamepadStreamSettings, PointerGate},
        gamepad_input_stream::gamepad_inputs,
        player_indicator_leds::PlayerIndicator,
    },
    network_connectivity::{ethernet_connection::Ethernet, wifi_connection::Wifi},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
};
use sottomano_toml_config::widget_groups::gamepad_bindings::{
    ButtonRole, CompositorAction, GamepadBindingsConfig, GamepadButton,
};
use uom::si::f64::Frequency;

use self::{
    connected_controller_reading::{ConnectedControllerReading, ControllerMarking},
    controller_battery_reading::ControllerBatteryReading,
    controller_link_reading::{BandSharing, ControllerLinkReading, WiredFallback, wifi_band},
    usb_cable_reading::{UsbCableReading, UsbCableRole},
};
use crate::{
    background_work::{compositor_request, logged_failure_task, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        reading_display_state::ReadingDisplayState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody, Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard},
        },
    },
    widget_groups::{
        gamepad_group::{
            controller_battery_reading::BatteryLevel, controller_link_reading::WifiBand,
        },
        widget_group_content::{
            KeyedExpansions, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        icon_glyph::IconGlyph,
        input_report_sections::MeasuredReportTiming,
        phosphor::{Phosphor, Weight},
    },
};

const CONTROLLER_DISCOVERY_GROUP: &str = "controller_discovery";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadTarget {
    Controller { slot: usize },
    Battery { slot: usize },
    Link { slot: usize },
    UsbCable { slot: usize },
    CompositorDriving { slot: usize },
    ControllerDiscovery,
}

impl GamepadTarget {
    #[must_use]
    pub const fn stable_item_name(&self) -> &'static str {
        match self {
            GamepadTarget::Controller { .. } => "controller",
            GamepadTarget::Battery { .. } => "battery",
            GamepadTarget::Link { .. } => "link",
            GamepadTarget::UsbCable { .. } => "usb_cable",
            GamepadTarget::CompositorDriving { .. } => "compositor_driving",
            GamepadTarget::ControllerDiscovery => "controller_discovery",
        }
    }
}

impl From<GamepadTarget> for HoverTarget {
    fn from(target: GamepadTarget) -> Self {
        HoverTarget::Gamepad(target)
    }
}

#[derive(Debug)]
pub enum GamepadMessage {
    Input(GamepadInput),
    DrivingControllersLoaded(BTreeSet<String>),
    KernelErrors(Vec<DeviceKernelError>),
    Wifi(Box<SnapshotDelivery<Wifi>>),
    Ethernet(Box<SnapshotDelivery<Ethernet>>),
    Interaction(GamepadInteraction),
}

#[derive(Debug, Clone)]
pub enum GamepadInteraction {
    DisconnectBluetooth {
        slot: usize,
    },
    ToggleCompositorDriving {
        slot: usize,
    },
    Expansion {
        slot: usize,
        expansion: WidgetGroupExpansionMessage,
    },
    ControllerDiscoveryExpansion(WidgetGroupExpansionMessage),
}

#[derive(Debug, Default)]
enum ControllerDiscovery {
    #[default]
    Watching,
    Failed(ChangeWatchFailure),
}

struct ControllerDiscoveryFailureReading<'failure> {
    discovery_failure: &'failure ChangeWatchFailure,
}

impl GroupItemReading for ControllerDiscoveryFailureReading<'_> {
    fn name(&self) -> &'static str {
        "Controller discovery"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::GameController(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some("failed".to_owned())
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        ReadingDisplayState::Failed
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([
            Section::error(self.discovery_failure),
            Section::note("New or removed controllers are not noticed until the panel restarts."),
        ])
    }
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
            Action::Dictation(command) => {
                Task::future(send_logged_dictation_command(command)).discard()
            }
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

#[derive(Debug, Clone, Default, PartialEq)]
enum WifiBandUse {
    #[default]
    ClearOfBluetooth,
    SharingBluetoothBand {
        network_name: String,
        frequency: Frequency,
    },
}

impl From<&Wifi> for WifiBandUse {
    fn from(wifi: &Wifi) -> Self {
        wifi.adapters
            .iter()
            .filter_map(|adapter| adapter.link.as_ref())
            .fold(WifiBandUse::ClearOfBluetooth, |band_use, link| {
                match (band_use, wifi_band(link.frequency)) {
                    (WifiBandUse::ClearOfBluetooth, WifiBand::SharedWithBluetooth) => {
                        WifiBandUse::SharingBluetoothBand {
                            network_name: link
                                .ssid
                                .clone()
                                .unwrap_or_else(|| "hidden network".to_owned()),
                            frequency: link.frequency,
                        }
                    }
                    (band_use, _) => band_use,
                }
            })
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
    controller_discovery_expansion: WidgetGroupExpansion,
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

fn compositor_driving_toggle(controller_custody: ControllerCustody) -> IconToggle {
    IconToggle::new(
        "Compositor control",
        Phosphor::AppWindow(Weight::Regular),
        controller_custody,
        SwitchPresses {
            turning_on: "let this controller drive the compositor again",
            turning_off: "hand this controller to the focused window",
        },
    )
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
            demand.while_rows_shown(
                snapshot_subscription::<Wifi>(buses.clone())
                    .map(|delivery| GamepadMessage::Wifi(Box::new(delivery))),
            ),
            demand.while_rows_shown(
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
                    Action::Compositor(compositor_request_of(action))
                }),
            GamepadMessage::KernelErrors(kernel_errors) => {
                self.kernel_errors = kernel_errors;
                Action::Idle
            }
            GamepadMessage::Wifi(delivery) => {
                match *delivery {
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Ok(wifi),
                        ..
                    } => self.wifi_band_use = WifiBandUse::from(&wifi),
                    SnapshotDelivery::SourceUnavailable
                    | SnapshotDelivery::InternetUnreachable(_)
                    | SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Err(_),
                        ..
                    } => self.wifi_band_use = WifiBandUse::ClearOfBluetooth,
                    SnapshotDelivery::SnapshotUnchanged => {}
                }
                Action::Idle
            }
            GamepadMessage::Ethernet(delivery) => {
                match *delivery {
                    SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Ok(ethernet),
                        ..
                    } => {
                        self.wired_fallback = if ethernet.connected_count() > 0 {
                            WiredFallback::Connected
                        } else {
                            WiredFallback::Absent
                        };
                    }
                    SnapshotDelivery::SourceUnavailable
                    | SnapshotDelivery::InternetUnreachable(_)
                    | SnapshotDelivery::SnapshotDelivered {
                        snapshot_fetch: Err(_),
                        ..
                    } => self.wired_fallback = WiredFallback::Absent,
                    SnapshotDelivery::SnapshotUnchanged => {}
                }
                Action::Idle
            }
            GamepadMessage::Interaction(interaction) => self.interact(interaction),
        }
    }

    fn interact(&mut self, interaction: GamepadInteraction) -> Action {
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
            && bindings.role_of(button) == Some(ButtonRole::PushToTalk);
        if !releasing_push_to_talk && self.controller_custody(slot) != ControllerCustody::Compositor
        {
            return Action::Idle;
        }
        bindings
            .role_of(button)
            .and_then(|role| role_request(role, edge))
            .unwrap_or(Action::Idle)
    }

    fn controller(&self, slot: usize) -> Option<&ConnectedGamepad> {
        self.controllers
            .iter()
            .find(|controller| controller.slot == slot)
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

    fn controllers_in_player_order(&self) -> Vec<(&ConnectedGamepad, &PhysicalGamepadLink)> {
        let mut ordered: Vec<_> = self.physical_controllers().collect();
        ordered.sort_by(
            |(left_controller, left_link), (right_controller, right_link)| {
                (&left_link.player_indicator, left_controller.slot)
                    .cmp(&(&right_link.player_indicator, right_controller.slot))
            },
        );
        ordered
    }

    fn controller_marking<'a>(
        &self,
        gamepad: &ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
    ) -> ControllerMarking<'a> {
        let unlit_controller_count = self
            .physical_controllers()
            .filter(|(_, other_link)| matches!(other_link.player_indicator, PlayerIndicator::Unlit))
            .count();
        match (&link.player_indicator, &link.identity) {
            (PlayerIndicator::Lit(lit_player), _) => ControllerMarking::PlayerLights(lit_player),
            (PlayerIndicator::Unlit, ControllerIdentity::Addressed { alias, .. })
                if unlit_controller_count > 1 && *alias != gamepad.name =>
            {
                ControllerMarking::PairedAlias(alias)
            }
            (PlayerIndicator::Unlit, _) => ControllerMarking::IconOnly,
        }
    }

    fn face_reading<'a>(
        &'a self,
        gamepad: &'a ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
        bindings: &'a GamepadBindingsConfig,
    ) -> ConnectedControllerReading<'a> {
        ConnectedControllerReading {
            gamepad,
            bindings,
            custody: self.controller_custody(gamepad.slot),
            marking: self.controller_marking(gamepad, link),
        }
    }

    fn link_reading<'a>(
        &'a self,
        slot: usize,
        link: &'a PhysicalGamepadLink,
    ) -> ControllerLinkReading<'a> {
        ControllerLinkReading {
            transport: link.transport,
            report_timing: self.report_timings.get(&slot).copied().map_or(
                MeasuredReportTiming::Pending,
                MeasuredReportTiming::Measured,
            ),
            kernel_errors: self
                .kernel_errors
                .iter()
                .filter(|error| error.kernel_device == link.kernel_device)
                .collect(),
            band_sharing: match &self.wifi_band_use {
                WifiBandUse::ClearOfBluetooth => BandSharing::NoConflict,
                WifiBandUse::SharingBluetoothBand {
                    network_name,
                    frequency,
                } => BandSharing::SharedWithWifi {
                    network_name,
                    frequency: *frequency,
                    wired_fallback: self.wired_fallback,
                },
            },
        }
    }

    fn hover_card_body(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        clock: Clock,
    ) -> Option<HoverCardBody> {
        Some(match target {
            GamepadTarget::Controller { slot } => self
                .face_reading(self.controller(slot)?, self.physical_link(slot)?, bindings)
                .hover_card(clock),
            GamepadTarget::Battery { slot } => ControllerBatteryReading {
                power: self.controller(slot)?.power,
            }
            .hover_card(clock),
            GamepadTarget::Link { slot } => self
                .link_reading(slot, self.physical_link(slot)?)
                .hover_card(clock),
            GamepadTarget::UsbCable { slot } => UsbCableReading {
                role: usb_cable_role(self.physical_link(slot)?.transport)?,
            }
            .hover_card(clock),
            GamepadTarget::CompositorDriving { slot } => {
                self.physical_link(slot)?;
                self.compositor_driving_hover_card(slot)
            }
            GamepadTarget::ControllerDiscovery => self
                .controller_discovery_failure_reading()?
                .hover_card(clock),
        })
    }

    fn compositor_driving_hover_card(&self, slot: usize) -> HoverCardBody {
        let (note_title, note_body) = match self.controller_custody(slot) {
            ControllerCustody::Compositor => (
                "Driving the compositor",
                "Buttons and sticks focus and move windows, whatever window has focus. Turn this \
                 off before playing a game: the controller then belongs to the focused window \
                 again.",
            ),
            ControllerCustody::HandedOver => (
                "Handed to the focused window",
                "Controllers start out belonging to the focused window, so games keep their \
                 input. Turn this on to focus and move windows with this controller; the choice \
                 is remembered for this controller.",
            ),
        };
        HoverCardBody::sections([Section::detailed_note(note_title, note_body)])
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        clock: Clock,
    ) -> Option<HoverCard> {
        self.hover_card_body(target, bindings, clock)
            .map(|body| HoverCard::sourced(body, DeliveryTiming::LivePush))
    }

    fn controller_discovery_failure_reading(
        &self,
    ) -> Option<ControllerDiscoveryFailureReading<'_>> {
        match &self.controller_discovery {
            ControllerDiscovery::Failed(discovery_failure) => {
                Some(ControllerDiscoveryFailureReading { discovery_failure })
            }
            ControllerDiscovery::Watching => None,
        }
    }

    fn controller_discovery_group(&self) -> Option<WidgetGroupContent<'_, GamepadInteraction>> {
        let failure_reading = self.controller_discovery_failure_reading()?;
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::Gamepad, CONTROLLER_DISCOVERY_GROUP),
            WidgetGroupTitle::Named(failure_reading.name().into()),
            &self.controller_discovery_expansion,
            GamepadInteraction::ControllerDiscoveryExpansion,
            [GroupItem::reading(
                failure_reading,
                GamepadTarget::ControllerDiscovery,
            )],
        ))
    }

    fn controller_group<'a>(
        &'a self,
        gamepad: &'a ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
        group_title: WidgetGroupTitle,
        bindings: &'a GamepadBindingsConfig,
    ) -> Option<WidgetGroupContent<'a, GamepadInteraction>> {
        let slot = gamepad.slot;
        let expansion = self.expansions.get(&slot)?;
        let battery_reading = ControllerBatteryReading {
            power: gamepad.power,
        };
        let face = GroupItem::reading(
            self.face_reading(gamepad, link, bindings),
            GamepadTarget::Controller { slot },
        );
        let shown_in_bluetooth_group = matches!(link.transport, GamepadTransport::Bluetooth { .. });
        let connection = (!shown_in_bluetooth_group).then(|| {
            GroupItem::reading(self.link_reading(slot, link), GamepadTarget::Link { slot })
        });
        let battery =
            (!shown_in_bluetooth_group && gamepad.power != GamepadPower::NoBattery).then(|| {
                let battery_running_low = battery_reading.battery_level() == BatteryLevel::Low;
                let battery_item =
                    GroupItem::reading(battery_reading, GamepadTarget::Battery { slot });
                if battery_running_low {
                    battery_item.previewed_on_handle()
                } else {
                    battery_item
                }
            });
        let compositor_driving = GroupItem::toggle(
            compositor_driving_toggle(self.controller_custody(slot)),
            GamepadTarget::CompositorDriving { slot },
            GamepadInteraction::ToggleCompositorDriving { slot },
        );
        let usb_cable = usb_cable_role(link.transport).map(|role| {
            let item =
                GroupItem::reading(UsbCableReading { role }, GamepadTarget::UsbCable { slot });
            match role {
                UsbCableRole::ChargingOnly => item.on_press(
                    "disconnect Bluetooth so the cable can take over",
                    GamepadInteraction::DisconnectBluetooth { slot },
                ),
                UsbCableRole::InputAndCharging => item,
            }
        });
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(
                WidgetGroupFamily::Gamepad,
                controller_custody_key(link).to_owned(),
            ),
            group_title,
            expansion,
            move |expansion| GamepadInteraction::Expansion { slot, expansion },
            [
                Some(face),
                Some(compositor_driving),
                connection,
                battery,
                usb_cable,
            ]
            .into_iter()
            .flatten(),
        ))
    }

    #[must_use]
    pub fn group_contents<'a>(
        &'a self,
        bindings: &'a GamepadBindingsConfig,
    ) -> Vec<WidgetGroupContent<'a, GamepadInteraction>> {
        let controllers = self.controllers_in_player_order();
        let titles = WidgetGroupTitle::numbered_namesakes(
            controllers.iter().map(|(gamepad, _)| gamepad.name.clone()),
        );
        controllers
            .iter()
            .zip(titles)
            .filter_map(|((gamepad, link), group_title)| {
                self.controller_group(gamepad, link, group_title, bindings)
            })
            .chain(self.controller_discovery_group())
            .collect()
    }
}

fn usb_cable_role(transport: GamepadTransport) -> Option<UsbCableRole> {
    match transport {
        GamepadTransport::Usb => Some(UsbCableRole::InputAndCharging),
        GamepadTransport::Bluetooth {
            usb_cable: UsbCableState::IgnoredByKernel,
        } => Some(UsbCableRole::ChargingOnly),
        GamepadTransport::Bluetooth {
            usb_cable: UsbCableState::Absent,
        }
        | GamepadTransport::Other => None,
    }
}

fn spawned(command: impl IntoIterator<Item = impl Into<String>>) -> CompositorRequest {
    CompositorRequest::Spawn(command.into_iter().map(Into::into).collect())
}

fn typed_key_command(key_chord: &str) -> Vec<String> {
    let (modifier_names, key_name) = key_chord.rsplit_once('+').unwrap_or(("", key_chord));
    let held_modifiers: Vec<&str> = modifier_names
        .split('+')
        .filter(|modifier_name| !modifier_name.is_empty())
        .collect();
    let pressed_modifiers = held_modifiers
        .iter()
        .flat_map(|modifier_name| ["-M", modifier_name]);
    let released_modifiers = held_modifiers
        .iter()
        .rev()
        .flat_map(|modifier_name| ["-m", modifier_name]);
    once("wtype")
        .chain(pressed_modifiers)
        .chain(["-k", key_name])
        .chain(released_modifiers)
        .map(str::to_owned)
        .collect()
}

fn role_request(role: ButtonRole<'_>, edge: ButtonEdge) -> Option<Action> {
    match (role, edge) {
        (ButtonRole::PushToTalk, ButtonEdge::Pressed) => {
            Some(Action::Dictation(DictationCommand::Start))
        }
        (ButtonRole::PushToTalk, ButtonEdge::Released) => {
            Some(Action::Dictation(DictationCommand::Stop))
        }
        (ButtonRole::TypedKey(key), ButtonEdge::Pressed) => {
            Some(Action::Compositor(spawned(typed_key_command(key))))
        }
        (ButtonRole::Compositor(action), ButtonEdge::Pressed) => {
            Some(Action::Compositor(compositor_request_of(action)))
        }
        (ButtonRole::Command(command), ButtonEdge::Pressed) => {
            Some(Action::Compositor(spawned(command.iter().cloned())))
        }
        (ButtonRole::PointerButton(_), _)
        | (
            ButtonRole::TypedKey(_) | ButtonRole::Compositor(_) | ButtonRole::Command(_),
            ButtonEdge::Released,
        ) => None,
    }
}

fn compositor_request_of(action: CompositorAction) -> CompositorRequest {
    match action {
        CompositorAction::FocusLeft => CompositorRequest::FocusLeft,
        CompositorAction::FocusRight => CompositorRequest::FocusRight,
        CompositorAction::FocusUp => CompositorRequest::FocusUp,
        CompositorAction::FocusDown => CompositorRequest::FocusDown,
        CompositorAction::FocusWorkspaceUp => CompositorRequest::FocusWorkspaceUp,
        CompositorAction::FocusWorkspaceDown => CompositorRequest::FocusWorkspaceDown,
        CompositorAction::FocusPreviousWindow => CompositorRequest::FocusPreviousWindow,
        CompositorAction::MoveLeft => CompositorRequest::MoveLeft,
        CompositorAction::MoveRight => CompositorRequest::MoveRight,
        CompositorAction::MoveWindowToWorkspaceUp => {
            CompositorRequest::MoveFocusedWindowToWorkspaceUp
        }
        CompositorAction::MoveWindowToWorkspaceDown => {
            CompositorRequest::MoveFocusedWindowToWorkspaceDown
        }
        CompositorAction::ToggleOverview => CompositorRequest::ToggleOverview,
        CompositorAction::Maximize => CompositorRequest::MaximizeFocused,
        CompositorAction::Center => CompositorRequest::CenterFocused,
        CompositorAction::CloseWindow => CompositorRequest::CloseFocusedWindow,
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::gamepad_controller_input::player_indicator_leds::LitPlayer;
    use sottomano_toml_config::widget_groups::gamepad_bindings::{GamepadStick, StickDirection};

    use super::*;

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
        group.update(
            GamepadMessage::Interaction(GamepadInteraction::ToggleCompositorDriving { slot }),
            &GamepadBindingsConfig::default(),
        )
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
            GamepadMessage::DrivingControllersLoaded(BTreeSet::from([
                "00:00:00:00:00:20".to_owned()
            ])),
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
                .group_contents(&GamepadBindingsConfig::default())
                .len(),
            2
        );
    }

    fn marking_of(group: &GamepadGroup, slot: usize) -> ControllerMarking<'_> {
        group.controller_marking(
            group.controller(slot).unwrap(),
            group.physical_link(slot).unwrap(),
        )
    }

    #[test]
    fn a_lit_pad_is_marked_by_its_player_number_instead_of_its_alias() {
        let group = group_with_two_pads_and_a_virtual_one();
        assert!(matches!(
            marking_of(&group, DUALSENSE),
            ControllerMarking::PlayerLights(LitPlayer { number: 2, .. })
        ));
        assert!(matches!(
            marking_of(&group, SECOND_PAD),
            ControllerMarking::PlayerLights(LitPlayer { number: 1, .. })
        ));
    }

    #[test]
    fn unlit_pads_fall_back_to_their_alias_only_when_ambiguous() {
        let mut group = GamepadGroup::default();
        let roster =
            |pads: Vec<ConnectedGamepad>| GamepadMessage::Input(GamepadInput::Roster(pads));
        let unlit_pad = |slot| connected_pad(slot, bluetooth_link(slot, PlayerIndicator::Unlit));
        group.update(
            roster(vec![unlit_pad(DUALSENSE)]),
            &GamepadBindingsConfig::default(),
        );
        assert!(matches!(
            marking_of(&group, DUALSENSE),
            ControllerMarking::IconOnly
        ));
        group.update(
            roster(vec![unlit_pad(DUALSENSE), unlit_pad(SECOND_PAD)]),
            &GamepadBindingsConfig::default(),
        );
        assert!(matches!(
            marking_of(&group, DUALSENSE),
            ControllerMarking::PairedAlias("Pad 31")
        ));
    }

    #[test]
    fn namesake_controllers_get_numbered_titles() {
        let titles: Vec<String> =
            WidgetGroupTitle::numbered_namesakes(["DualSense", "Xbox", "DualSense"])
                .iter()
                .map(ToString::to_string)
                .collect();
        assert_eq!(titles, ["DualSense", "Xbox", "DualSense 2"]);
    }

    #[test]
    fn a_controller_item_name_ignores_its_slot() {
        assert_eq!(
            GamepadTarget::Battery { slot: 0 }.stable_item_name(),
            GamepadTarget::Battery { slot: 3 }.stable_item_name()
        );
    }
}
