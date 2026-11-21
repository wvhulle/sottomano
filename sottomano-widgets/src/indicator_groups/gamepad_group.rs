mod connected_controller_indicator;
mod controller_battery_indicator;
pub mod controller_button_labels;
mod controller_link_indicator;
mod diagram_anchor_mapping;
mod sqlite_compositor_driving_controllers;
mod steam_input_route;
mod steam_radio_firmware_indicators;
mod usb_cable_indicator;
use std::{
    collections::{BTreeMap, BTreeSet},
    iter,
    iter::once,
};

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
    delivery_cadence_drivers::ChangeWatchFailure,
    gamepad_controller_input::{
        accelerometer_report_timing::ReportTiming,
        evdev_gamepad_devices::{
            ButtonEdge, ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadPower,
            GamepadTransport, InputRoute, PhysicalGamepadLink, UsbCableState,
        },
        gamepad_input_machine::{GamepadInput, GamepadStreamSettings, PointerGate},
        gamepad_input_stream::gamepad_inputs,
        player_indicator_leds::PlayerIndicator,
        steam_input_facts::{RadioReading, SteamInputFacts},
    },
    network_connectivity::{ethernet_connection::Ethernet, wifi_connection::Wifi},
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
};
use sottomano_toml_config::indicator_groups::gamepad_bindings::{
    ButtonRole, CompositorAction, GamepadBindingsConfig, GamepadButton,
};
use uom::si::f64::Frequency;

use self::{
    connected_controller_indicator::{ConnectedControllerIndicator, ControllerMarking},
    controller_battery_indicator::ControllerBatteryIndicator,
    controller_link_indicator::{BandSharing, ControllerLinkIndicator, WiredFallback, wifi_band},
    steam_radio_firmware_indicators::{SteamFirmwareIndicator, SteamRadioIndicator},
    usb_cable_indicator::{UsbCableIndicator, UsbCableRole},
};
use crate::{
    background_work::{compositor_request, logged_failure_task, snapshot_subscription},
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
        toggle_control::{IconToggle, SwitchPosition, SwitchPresses},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::hover_card_body::{Section, SectionWidget},
    indicator_groups::{
        gamepad_group::controller_link_indicator::WifiBand,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
            KeyedExpansions,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::input_report_sections::MeasuredReportTiming,
};

const CONTROLLER_DISCOVERY_GROUP: &str = "controller_discovery";

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

#[derive(Debug, Default)]
enum ControllerDiscovery {
    #[default]
    Watching,
    Failed(ChangeWatchFailure),
}

struct ControllerDiscoveryFailureIndicator<'failure> {
    discovery_failure: &'failure ChangeWatchFailure,
}

impl<'failure> ControllerDiscoveryFailureIndicator<'failure> {
    fn indicator(self) -> GroupIndicator<'failure> {
        GroupIndicator::glyph_only(
            "Controller discovery",
            Phosphor::GameController(Weight::Regular).into(),
            IndicatorState::Failed,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("failed".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Error",
                SectionWidget::paragraph(self.discovery_failure.to_string()),
            ),
            Section::new(
                "Hotplug",
                SectionWidget::paragraph(
                    "New or removed controllers are not noticed until the panel restarts.",
                ),
            ),
        ]
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
                    Action::Compositor(compositor_request_of(action))
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

    fn receive_wifi_delivery(&mut self, delivery: SnapshotDelivery<Wifi>) {
        match delivery {
            SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Ok(wifi),
                ..
            } => self.wifi_band_use = WifiBandUse::from(&wifi),
            SnapshotDelivery::SourceUnavailable
            | SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Err(_),
                ..
            } => self.wifi_band_use = WifiBandUse::ClearOfBluetooth,
            SnapshotDelivery::SnapshotUnchanged => {}
        }
    }

    fn receive_ethernet_delivery(&mut self, delivery: SnapshotDelivery<Ethernet>) {
        match delivery {
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
            | SnapshotDelivery::SnapshotDelivered {
                snapshot_fetch: Err(_),
                ..
            } => self.wired_fallback = WiredFallback::Absent,
            SnapshotDelivery::SnapshotUnchanged => {}
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

    fn face_indicator<'a>(
        &'a self,
        gamepad: &'a ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
        bindings: &'a GamepadBindingsConfig,
    ) -> ConnectedControllerIndicator<'a> {
        ConnectedControllerIndicator {
            gamepad,
            bindings,
            custody: self.controller_custody(gamepad.slot),
            marking: self.controller_marking(gamepad, link),
        }
    }

    fn link_indicator<'a>(
        &'a self,
        slot: usize,
        link: &'a PhysicalGamepadLink,
    ) -> ControllerLinkIndicator<'a> {
        ControllerLinkIndicator {
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

    fn compositor_driving_hover_card(&self, slot: usize) -> Vec<Section> {
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
        vec![Section::new(
            note_title,
            SectionWidget::paragraph(note_body),
        )]
    }

    fn controller_discovery_failure_indicator(
        &self,
    ) -> Option<ControllerDiscoveryFailureIndicator<'_>> {
        match &self.controller_discovery {
            ControllerDiscovery::Failed(discovery_failure) => {
                Some(ControllerDiscoveryFailureIndicator { discovery_failure })
            }
            ControllerDiscovery::Watching => None,
        }
    }

    fn controller_discovery_group(&self) -> Option<IndicatorGroupContent<'_>> {
        let listed_failure = self.controller_discovery_failure_indicator()?.indicator();
        let group_title = IndicatorGroupTitle::Named(listed_failure.indicator_name());
        let headline_indicator = self
            .controller_discovery_failure_indicator()?
            .indicator()
            .with_headline_glyph(Phosphor::Joystick(Weight::Regular).into());
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::Gamepad,
                CONTROLLER_DISCOVERY_GROUP,
            ),
            group_title,
            headline_indicator,
            &self.controller_discovery_expansion,
            GamepadInteraction::ControllerDiscoveryExpansion,
            [listed_failure],
        ))
    }

    fn controller_group<'a>(
        &'a self,
        gamepad: &'a ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
        group_title: IndicatorGroupTitle,
        bindings: &'a GamepadBindingsConfig,
    ) -> Option<IndicatorGroupContent<'a>> {
        let slot = gamepad.slot;
        let expansion = self.expansions.expansions.get(&slot)?;
        let battery_indicator = ControllerBatteryIndicator {
            power: gamepad.power,
        };
        let build_controller_indicator =
            || self.face_indicator(gamepad, link, bindings).indicator();
        let headline_indicator = build_controller_indicator()
            .with_headline_glyph(Phosphor::Joystick(Weight::Regular).into());
        let shown_in_bluetooth_group = matches!(link.transport, GamepadTransport::Bluetooth { .. });
        let mut rows = vec![
            GroupIndicator::toggle(
                compositor_driving_toggle(self.controller_custody(slot)),
                GamepadInteraction::ToggleCompositorDriving { slot },
            )
            .with_hover_card(Box::new(move |_| self.compositor_driving_hover_card(slot))),
        ];
        if !shown_in_bluetooth_group {
            match link.input_route {
                InputRoute::Kernel => rows.push(self.link_indicator(slot, link).indicator()),
                InputRoute::SteamInput(SteamInputFacts {
                    radio: RadioReading::Measured(signal),
                    ..
                }) => rows.push(SteamRadioIndicator { signal }.indicator()),
                InputRoute::SteamInput(_) => {}
            }
            if gamepad.power != GamepadPower::NoBattery {
                rows.push(battery_indicator.indicator());
            }
        }
        if let InputRoute::SteamInput(facts) = link.input_route
            && SteamFirmwareIndicator::has_pending_update(&facts)
        {
            rows.push(SteamFirmwareIndicator { facts }.indicator());
        }
        if let Some(cable_role) = usb_cable_role(link.transport) {
            let cable_indicator = UsbCableIndicator { role: cable_role }.indicator();
            rows.push(match cable_role {
                UsbCableRole::ChargingOnly => cable_indicator.on_press(
                    "disconnect Bluetooth so the cable can take over",
                    GamepadInteraction::DisconnectBluetooth { slot },
                ),
                UsbCableRole::InputAndCharging => cable_indicator,
            });
        }
        Some(IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::Gamepad,
                controller_custody_key(link).to_owned(),
            ),
            group_title,
            headline_indicator,
            expansion,
            move |expansion| GamepadInteraction::Expansion { slot, expansion },
            iter::once(build_controller_indicator()).chain(rows),
        ))
    }

    #[must_use]
    pub fn group_contents<'a>(
        &'a self,
        bindings: &'a GamepadBindingsConfig,
    ) -> Vec<IndicatorGroupContent<'a>> {
        let controllers = self.controllers_in_player_order();
        let titles = IndicatorGroupTitle::numbered_namesakes(
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
        | GamepadTransport::SteamDongle
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
    use sottomano_toml_config::indicator_groups::gamepad_bindings::{GamepadStick, StickDirection};

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
        group.interact(GamepadInteraction::ToggleCompositorDriving { slot })
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
            product_id: 0x0ce6,
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
            input_route: InputRoute::Kernel,
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
            group
                .controllers
                .iter()
                .find(|controller| controller.slot == slot)
                .unwrap(),
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
            IndicatorGroupTitle::numbered_namesakes(["DualSense", "Xbox", "DualSense"])
                .iter()
                .map(ToString::to_string)
                .collect();
        assert_eq!(titles, ["DualSense", "Xbox", "DualSense 2"]);
    }
}
