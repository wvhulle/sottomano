mod connected_controller_reading;
mod controller_battery_reading;
pub mod controller_button_labels;
mod controller_link_reading;
mod diagram_anchor_mapping;
mod usb_cable_reading;

use std::{
    collections::{BTreeMap, BTreeSet},
    iter::once,
};

use iced::{Subscription, Task};
use notulist_client::DictationCommand;
use sottomano_desktop_sinks::{
    desktop_windows_apps::compositor_requests::{CompositorRequest, CompositorRequestFailure},
    network_connectivity::bluez_bluetooth_commands::disconnect_bluetooth_address,
    voice_dictation_input::notulist_dictation_commands::send_logged_dictation_command,
};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    gamepad_controller_input::{
        accelerometer_report_timing::ReportTiming,
        evdev_gamepad_devices::{
            ButtonEdge, ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadPower,
            GamepadTransport, PhysicalGamepadLink, UsbCableState,
        },
        gamepad_input_machine::{ClaimSilencing, GamepadInput, GamepadStreamSettings, PointerGate},
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
    ButtonRole, CompositorAction, FocusBehaviour, GamepadBindingsConfig, GamepadButton,
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
        widget_group_content::{KeyedExpansions, WidgetGroupContent, WidgetGroupExpansionMessage},
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        input_report_sections::MeasuredReportTiming,
        phosphor::{Phosphor, Weight},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadTarget {
    Controller { slot: usize },
    Battery { slot: usize },
    Link { slot: usize },
    UsbCable { slot: usize },
    CompositorDriving { slot: usize },
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
        }
    }
}

impl From<GamepadTarget> for HoverTarget {
    fn from(target: GamepadTarget) -> Self {
        HoverTarget::Gamepad(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorControl<'a> {
    DrivingCompositor,
    HandedOverByHand,
    PausedForFocusedApp(&'a str),
    PausedForFullscreenApp(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadWindowClaim<'a> {
    Unclaimed,
    Focused(&'a str),
    Fullscreen(&'a str),
}

impl GamepadWindowClaim<'_> {
    fn claim_silencing(self, bindings: &GamepadBindingsConfig) -> ClaimSilencing {
        match self {
            Self::Unclaimed => ClaimSilencing::Free,
            Self::Focused(app_id) => match bindings.focus_behaviour(Some(app_id)) {
                FocusBehaviour::Passive => ClaimSilencing::Silenced,
                FocusBehaviour::Active => ClaimSilencing::Free,
            },
            Self::Fullscreen(_) => ClaimSilencing::Silenced,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GamepadMessage {
    Input(GamepadInput),
    KernelErrors(Vec<DeviceKernelError>),
    Wifi(Box<SnapshotDelivery<Wifi>>),
    Ethernet(Box<SnapshotDelivery<Ethernet>>),
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
}

#[derive(Debug, Clone)]
pub enum Action {
    Idle,
    Compositor(CompositorRequest),
    Dictation(DictationCommand),
    DisconnectBluetooth { hardware_address: String },
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
    handed_over_controllers: BTreeSet<String>,
}

fn handover_key(link: &PhysicalGamepadLink) -> &str {
    match &link.identity {
        ControllerIdentity::Addressed {
            hardware_address, ..
        } => hardware_address,
        ControllerIdentity::Unaddressed => &link.kernel_device,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControllerCustody {
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
    }

    fn controller_custody(&self, slot: usize) -> ControllerCustody {
        match self.physical_link(slot) {
            Some(link) if self.handed_over_controllers.contains(handover_key(link)) => {
                ControllerCustody::HandedOver
            }
            _ => ControllerCustody::Compositor,
        }
    }

    fn control<'a>(
        &self,
        slot: usize,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'a>,
    ) -> CompositorControl<'a> {
        if self.controller_custody(slot) == ControllerCustody::HandedOver {
            return CompositorControl::HandedOverByHand;
        }
        match window_claim {
            GamepadWindowClaim::Fullscreen(app_id) => {
                CompositorControl::PausedForFullscreenApp(app_id)
            }
            GamepadWindowClaim::Focused(app_id)
                if window_claim.claim_silencing(bindings) == ClaimSilencing::Silenced =>
            {
                CompositorControl::PausedForFocusedApp(app_id)
            }
            GamepadWindowClaim::Unclaimed | GamepadWindowClaim::Focused(_) => {
                CompositorControl::DrivingCompositor
            }
        }
    }

    #[must_use]
    pub fn stream_settings(
        &self,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
    ) -> GamepadStreamSettings {
        GamepadStreamSettings {
            steering: bindings.pointer_steering(),
            gate: PointerGate {
                claim_silencing: window_claim.claim_silencing(bindings),
                handed_over_slots: self
                    .controllers
                    .iter()
                    .map(|controller| controller.slot)
                    .filter(|slot| self.controller_custody(*slot) == ControllerCustody::HandedOver)
                    .collect(),
            },
        }
    }

    pub fn subscription(
        &self,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
    ) -> Subscription<GamepadMessage> {
        Subscription::batch([
            Subscription::run_with(self.stream_settings(bindings, window_claim), |settings| {
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

    pub fn update(
        &mut self,
        message: GamepadMessage,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
    ) -> Action {
        match message {
            GamepadMessage::Input(GamepadInput::Roster(controllers)) => {
                self.receive_roster(controllers);
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::ReportTiming { slot, timing }) => {
                self.report_timings.insert(slot, timing);
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::Button { slot, button, edge }) => {
                self.receive_button(slot, button, edge, bindings, window_claim)
            }
            GamepadMessage::Input(GamepadInput::StickFlick {
                slot,
                stick,
                direction,
            }) => bindings
                .navigation
                .flick_action(stick, direction)
                .filter(|_| {
                    self.control(slot, bindings, window_claim)
                        == CompositorControl::DrivingCompositor
                })
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
            GamepadMessage::DisconnectBluetooth { slot } => {
                self.bluetooth_address(slot)
                    .map_or(Action::Idle, |hardware_address| {
                        Action::DisconnectBluetooth {
                            hardware_address: hardware_address.to_owned(),
                        }
                    })
            }
            GamepadMessage::ToggleCompositorDriving { slot } => {
                if let Some(key) = self.physical_link(slot).map(handover_key) {
                    let key = key.to_owned();
                    if !self.handed_over_controllers.remove(&key) {
                        self.handed_over_controllers.insert(key);
                    }
                }
                Action::Idle
            }
            GamepadMessage::Expansion { slot, expansion } => {
                self.expansions.update(&slot, expansion);
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
        window_claim: GamepadWindowClaim<'_>,
    ) -> Action {
        let releasing_push_to_talk = edge == ButtonEdge::Released
            && bindings.role_of(button) == Some(ButtonRole::PushToTalk);
        if !releasing_push_to_talk
            && self.control(slot, bindings, window_claim) != CompositorControl::DrivingCompositor
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
        window_claim: GamepadWindowClaim<'a>,
    ) -> ConnectedControllerReading<'a> {
        ConnectedControllerReading {
            gamepad,
            bindings,
            control: self.control(gamepad.slot, bindings, window_claim),
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
        window_claim: GamepadWindowClaim<'_>,
        clock: Clock,
    ) -> Option<HoverCardBody> {
        Some(match target {
            GamepadTarget::Controller { slot } => self
                .face_reading(
                    self.controller(slot)?,
                    self.physical_link(slot)?,
                    bindings,
                    window_claim,
                )
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
                self.compositor_driving_hover_card(slot, bindings, window_claim)
            }
        })
    }

    fn compositor_driving_hover_card(
        &self,
        slot: usize,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
    ) -> HoverCardBody {
        let (note_title, note_body) = match self.control(slot, bindings, window_claim) {
            CompositorControl::DrivingCompositor => (
                "Driving the compositor",
                "Buttons and sticks focus and move windows. Turn this off when a game keeps \
                 losing its window to the compositor: the controller then belongs to the focused \
                 window until you turn it back on."
                    .to_owned(),
            ),
            CompositorControl::HandedOverByHand => (
                "Handed to the focused window",
                "Compositor bindings stay off for this controller, whatever window has focus, \
                 until you turn this back on."
                    .to_owned(),
            ),
            CompositorControl::PausedForFocusedApp(app_id) => (
                "Paused while a game has focus",
                format!(
                    "{app_id} has focus, so compositor bindings already pause on their own. \
                     Turning this off keeps them off after focus moves."
                ),
            ),
            CompositorControl::PausedForFullscreenApp(app_id) => (
                "Paused while a window fills the screen",
                format!(
                    "{app_id} fills the screen, so compositor bindings pause even while a prompt \
                     has focus. Turning this off keeps them off after it leaves fullscreen."
                ),
            ),
        };
        HoverCardBody::sections([Section::detailed_note(note_title, note_body)])
    }

    #[must_use]
    pub fn hover_card_for(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
        clock: Clock,
    ) -> Option<HoverCard> {
        self.hover_card_body(target, bindings, window_claim, clock)
            .map(|body| HoverCard::sourced(body, DeliveryTiming::LivePush))
    }

    fn controller_group<'a>(
        &'a self,
        gamepad: &'a ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
        group_title: WidgetGroupTitle,
        bindings: &'a GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'a>,
    ) -> Option<WidgetGroupContent<'a, GamepadMessage>> {
        let slot = gamepad.slot;
        let expansion = self.expansions.get(&slot)?;
        let battery_reading = ControllerBatteryReading {
            power: gamepad.power,
        };
        let face = GroupItem::reading(
            self.face_reading(gamepad, link, bindings, window_claim),
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
            GamepadMessage::ToggleCompositorDriving { slot },
        );
        let usb_cable = usb_cable_role(link.transport).map(|role| {
            let item =
                GroupItem::reading(UsbCableReading { role }, GamepadTarget::UsbCable { slot });
            match role {
                UsbCableRole::ChargingOnly => item.on_press(
                    "disconnect Bluetooth so the cable can take over",
                    GamepadMessage::DisconnectBluetooth { slot },
                ),
                UsbCableRole::InputAndCharging => item,
            }
        });
        Some(WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::Gamepad, handover_key(link).to_owned()),
            group_title,
            expansion,
            move |expansion| GamepadMessage::Expansion { slot, expansion },
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
        window_claim: GamepadWindowClaim<'a>,
    ) -> Vec<WidgetGroupContent<'a, GamepadMessage>> {
        let controllers = self.controllers_in_player_order();
        let titles = WidgetGroupTitle::numbered_namesakes(
            controllers.iter().map(|(gamepad, _)| gamepad.name.clone()),
        );
        controllers
            .iter()
            .zip(titles)
            .filter_map(|((gamepad, link), group_title)| {
                self.controller_group(gamepad, link, group_title, bindings, window_claim)
            })
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
    use sottomano_desktop_sources::gamepad_controller_input::{
        gamepad_input_machine::ClaimSilencing, player_indicator_leds::LitPlayer,
    };
    use sottomano_toml_config::widget_groups::gamepad_bindings::{GamepadStick, StickDirection};

    use super::*;

    const DUALSENSE: usize = 31;
    const SECOND_PAD: usize = 32;

    fn button(slot: usize, button: GamepadButton, edge: ButtonEdge) -> GamepadMessage {
        GamepadMessage::Input(GamepadInput::Button { slot, button, edge })
    }

    fn press(group: &mut GamepadGroup, slot: usize, pressed: GamepadButton) -> Action {
        group.update(
            button(slot, pressed, ButtonEdge::Pressed),
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        )
    }

    fn release(group: &mut GamepadGroup, slot: usize, released: GamepadButton) -> Action {
        group.update(
            button(slot, released, ButtonEdge::Released),
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        )
    }

    fn spawned_command(action: Action) -> Vec<String> {
        let Action::Compositor(CompositorRequest::Spawn(command)) = action else {
            panic!("expected a spawned command, got {action:?}");
        };
        command
    }

    #[test]
    fn dpad_left_types_the_left_arrow_key_by_default() {
        let action = press(
            &mut GamepadGroup::default(),
            DUALSENSE,
            GamepadButton::DpadLeft,
        );
        assert_eq!(spawned_command(action), ["wtype", "-k", "Left"]);
    }

    #[test]
    fn triangle_types_shift_tab_by_default() {
        let action = press(
            &mut GamepadGroup::default(),
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
        let action = GamepadGroup::default().update(
            GamepadMessage::Input(GamepadInput::StickFlick {
                slot: DUALSENSE,
                stick: GamepadStick::Left,
                direction: StickDirection::Right,
            }),
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        assert!(matches!(
            action,
            Action::Compositor(CompositorRequest::FocusRight)
        ));
    }

    #[test]
    fn a_focused_game_keeps_its_controller_input() {
        let action = GamepadGroup::default().update(
            button(DUALSENSE, GamepadButton::DpadLeft, ButtonEdge::Pressed),
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("steam_app_1091500"),
        );
        assert!(matches!(action, Action::Idle));
    }

    #[test]
    fn holding_the_left_trigger_dictates_until_released() {
        let mut group = GamepadGroup::default();
        let started = press(&mut group, DUALSENSE, GamepadButton::LeftTrigger);
        assert!(matches!(
            started,
            Action::Dictation(DictationCommand::Start)
        ));
        let stopped = release(&mut group, DUALSENSE, GamepadButton::LeftTrigger);
        assert!(matches!(stopped, Action::Dictation(DictationCommand::Stop)));
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

    const STEAM_VIRTUAL_PAD: usize = 261;

    fn group_with_two_pads_and_a_virtual_one() -> GamepadGroup {
        let mut group = GamepadGroup::default();
        group.update(
            GamepadMessage::Input(GamepadInput::Roster(vec![
                connected_pad(DUALSENSE, bluetooth_link(DUALSENSE, lit_player(2))),
                connected_pad(SECOND_PAD, bluetooth_link(SECOND_PAD, lit_player(1))),
                connected_pad(STEAM_VIRTUAL_PAD, GamepadOrigin::Virtual),
            ])),
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        group
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
                .group_contents(
                    &GamepadBindingsConfig::default(),
                    GamepadWindowClaim::Focused("kitty")
                )
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
            GamepadWindowClaim::Focused("kitty"),
        );
        assert!(matches!(
            marking_of(&group, DUALSENSE),
            ControllerMarking::IconOnly
        ));
        group.update(
            roster(vec![unlit_pad(DUALSENSE), unlit_pad(SECOND_PAD)]),
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        assert!(matches!(
            marking_of(&group, DUALSENSE),
            ControllerMarking::PairedAlias("Pad 31")
        ));
    }

    #[test]
    fn controllers_drive_the_compositor_unless_a_game_has_focus() {
        let bindings = GamepadBindingsConfig::default();
        let group = group_with_two_pads_and_a_virtual_one();
        assert_eq!(
            group.control(DUALSENSE, &bindings, GamepadWindowClaim::Focused("kitty")),
            CompositorControl::DrivingCompositor
        );
        assert_eq!(
            group.control(
                DUALSENSE,
                &bindings,
                GamepadWindowClaim::Focused("steam_app_1091500")
            ),
            CompositorControl::PausedForFocusedApp("steam_app_1091500")
        );
    }

    #[test]
    fn a_handed_over_controller_stops_driving_the_compositor_while_others_continue() {
        let mut group = group_with_two_pads_and_a_virtual_one();
        group.update(
            GamepadMessage::ToggleCompositorDriving { slot: DUALSENSE },
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        assert!(matches!(
            press(&mut group, DUALSENSE, GamepadButton::DpadLeft),
            Action::Idle
        ));
        assert!(matches!(
            press(&mut group, SECOND_PAD, GamepadButton::DpadLeft),
            Action::Compositor(_)
        ));
        let settings = group.stream_settings(
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        assert_eq!(settings.gate.handed_over_slots, BTreeSet::from([DUALSENSE]));
        group.update(
            GamepadMessage::ToggleCompositorDriving { slot: DUALSENSE },
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        assert!(matches!(
            press(&mut group, DUALSENSE, GamepadButton::DpadLeft),
            Action::Compositor(_)
        ));
    }

    #[test]
    fn a_handover_survives_the_controller_reconnecting_on_a_new_slot() {
        let mut group = group_with_two_pads_and_a_virtual_one();
        group.update(
            GamepadMessage::ToggleCompositorDriving { slot: DUALSENSE },
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        let reconnected_slot = 40;
        group.update(
            GamepadMessage::Input(GamepadInput::Roster(vec![connected_pad(
                reconnected_slot,
                bluetooth_link(DUALSENSE, lit_player(1)),
            )])),
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("kitty"),
        );
        assert_eq!(
            group.control(
                reconnected_slot,
                &GamepadBindingsConfig::default(),
                GamepadWindowClaim::Focused("kitty")
            ),
            CompositorControl::HandedOverByHand
        );
    }

    #[test]
    fn a_fullscreen_window_pauses_controllers_without_a_passive_app_id() {
        let bindings = GamepadBindingsConfig::default();
        let group = group_with_two_pads_and_a_virtual_one();
        let claim = GamepadWindowClaim::Fullscreen("slowroads");
        assert_eq!(
            group.control(DUALSENSE, &bindings, claim),
            CompositorControl::PausedForFullscreenApp("slowroads")
        );
        assert_eq!(
            group.stream_settings(&bindings, claim).gate.claim_silencing,
            ClaimSilencing::Silenced
        );
    }

    #[test]
    fn an_unclaimed_desktop_leaves_controllers_driving_the_compositor() {
        let bindings = GamepadBindingsConfig::default();
        let group = group_with_two_pads_and_a_virtual_one();
        assert_eq!(
            group.control(DUALSENSE, &bindings, GamepadWindowClaim::Unclaimed),
            CompositorControl::DrivingCompositor
        );
    }

    #[test]
    fn focusing_a_game_silences_the_pointer_through_the_stream_settings() {
        let settings = GamepadGroup::default().stream_settings(
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("steam_app_1091500"),
        );
        assert_eq!(settings.gate.claim_silencing, ClaimSilencing::Silenced);
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
