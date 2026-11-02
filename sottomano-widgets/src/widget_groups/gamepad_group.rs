mod connected_controller_reading;
mod controller_battery_reading;
pub mod controller_button_labels;
mod controller_link_reading;
mod diagram_anchor_mapping;
mod usb_cable_reading;

use std::collections::{BTreeMap, BTreeSet};

use iced::{Subscription, Task};
use notulist_client::DictationCommand;
use sottomano_config::widget_groups::gamepad_bindings::{
    ButtonRole, CompositorAction, GamepadBindingsConfig, GamepadButton,
};
use sottomano_sources::{
    SnapshotDelivery,
    desktop_windows_apps::niri_action_requests::ActionFailure,
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
    network_connectivity::{
        bluez_bluetooth::disconnect_bluetooth_address, ethernet_connection::Ethernet,
        wifi_connection::Wifi,
    },
    system_protocol_clients::dbus_desktop_buses::DesktopBuses,
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
    voice_dictation_input::notulist_dbus_dictation::send_logged_dictation_command,
};
use uom::si::f64::Frequency;

use self::{
    connected_controller_reading::{ConnectedControllerReading, ControllerMarking},
    controller_battery_reading::ControllerBatteryReading,
    controller_link_reading::{
        BandSharing, ControllerLinkReading, WiredFallback, shares_bluetooth_band,
    },
    usb_cable_reading::{UsbCableReading, UsbCableRole},
};
use crate::{
    background_work::{niri_action, snapshot_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::group_menus::group_source_demand::GroupSourceDemand,
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        input_report_sections::MeasuredReportTiming,
        phosphor::{Phosphor, Weight},
        toggle_control::{IconToggle, SwitchPresses},
        tooltip_body::{
            Section, TooltipBody,
            tooltip_source_delivery::{DeliveryTiming, Tooltip},
        },
        widget_group::{
            WidgetGroup, WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadTarget {
    Controller { slot: usize },
    Battery { slot: usize },
    Link { slot: usize },
    UsbCable { slot: usize },
    NiriControl { slot: usize },
}

impl From<GamepadTarget> for HoverTarget {
    fn from(target: GamepadTarget) -> Self {
        HoverTarget::Gamepad(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorControl<'a> {
    DrivingNiri,
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
    fn silences_controllers(self, bindings: &GamepadBindingsConfig) -> bool {
        match self {
            Self::Unclaimed => false,
            Self::Focused(app_id) => !bindings.acts_while_focused(Some(app_id)),
            Self::Fullscreen(_) => true,
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
    ToggleNiriControl {
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
    Compositor(niri_ipc::Action),
    Dictation(DictationCommand),
    DisconnectBluetooth { hardware_address: String },
}

impl Action {
    pub fn into_task<M: From<ActionFailure> + Send + 'static>(self) -> Task<M> {
        match self {
            Action::Idle => Task::none(),
            Action::Compositor(action) => niri_action(action),
            Action::Dictation(command) => {
                Task::future(send_logged_dictation_command(command)).discard()
            }
            Action::DisconnectBluetooth { hardware_address } => Task::future(async move {
                if let Err(bus_error) = disconnect_bluetooth_address(hardware_address).await {
                    tracing::warn!("disconnect Bluetooth controller: {bus_error}");
                }
            })
            .discard(),
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

impl WifiBandUse {
    fn of_wifi(wifi: &Wifi) -> Self {
        wifi.adapters
            .iter()
            .filter_map(|adapter| adapter.link.as_ref())
            .find(|link| shares_bluetooth_band(link.frequency))
            .map_or(WifiBandUse::ClearOfBluetooth, |link| {
                WifiBandUse::SharingBluetoothBand {
                    network_name: link
                        .ssid
                        .clone()
                        .unwrap_or_else(|| "hidden network".to_owned()),
                    frequency: link.frequency,
                }
            })
    }
}

#[derive(Default)]
struct ControllerSession {
    expansion: WidgetGroupExpansion,
}

#[derive(Default)]
pub struct GamepadGroup {
    controllers: Vec<ConnectedGamepad>,
    sessions: BTreeMap<usize, ControllerSession>,
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

fn niri_control_toggle(driving_niri: bool) -> IconToggle {
    IconToggle::new(
        "Niri control",
        Phosphor::AppWindow(Weight::Regular),
        driving_niri,
        SwitchPresses {
            turning_on: "let this controller drive niri again",
            turning_off: "hand this controller to the focused window",
        },
    )
}

impl WidgetGroup for GamepadGroup {
    const TITLE: &'static str = "Gamepads";
}

impl GamepadGroup {
    pub fn collapse(&mut self) {
        for session in self.sessions.values_mut() {
            session.expansion.collapse();
        }
    }

    fn handed_over(&self, slot: usize) -> bool {
        self.physical_link(slot)
            .is_some_and(|link| self.handed_over_controllers.contains(handover_key(link)))
    }

    fn control<'a>(
        &self,
        slot: usize,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'a>,
    ) -> CompositorControl<'a> {
        if self.handed_over(slot) {
            return CompositorControl::HandedOverByHand;
        }
        match window_claim {
            GamepadWindowClaim::Fullscreen(app_id) => {
                CompositorControl::PausedForFullscreenApp(app_id)
            }
            GamepadWindowClaim::Focused(app_id) if window_claim.silences_controllers(bindings) => {
                CompositorControl::PausedForFocusedApp(app_id)
            }
            GamepadWindowClaim::Unclaimed | GamepadWindowClaim::Focused(_) => {
                CompositorControl::DrivingNiri
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
                silenced_by_window_claim: window_claim.silences_controllers(bindings),
                handed_over_slots: self
                    .controllers
                    .iter()
                    .map(|controller| controller.slot)
                    .filter(|slot| self.handed_over(*slot))
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
        demand: GroupSourceDemand<'_>,
    ) -> Subscription<GamepadMessage> {
        let any_session_expanded = self
            .sessions
            .values()
            .any(|session| session.expansion.is_expanded());
        let demand = demand.expanded_when(any_session_expanded);
        Subscription::batch([
            demand.while_rows_shown(
                &[controller_link_reading::CONTROLLER_LAG_ITEM_NAME],
                snapshot_subscription::<Wifi>(buses.clone())
                    .map(|delivery| GamepadMessage::Wifi(Box::new(delivery))),
            ),
            demand.while_rows_shown(
                &[controller_link_reading::CONTROLLER_LAG_ITEM_NAME],
                snapshot_subscription::<Ethernet>(buses.clone())
                    .map(|delivery| GamepadMessage::Ethernet(Box::new(delivery))),
            ),
        ])
    }

    fn acts(
        &self,
        slot: usize,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
    ) -> bool {
        self.control(slot, bindings, window_claim) == CompositorControl::DrivingNiri
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
                .filter(|_| self.acts(slot, bindings, window_claim))
                .map_or(Action::Idle, |action| {
                    Action::Compositor(niri_request(action))
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
                    } => self.wifi_band_use = WifiBandUse::of_wifi(&wifi),
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
            GamepadMessage::ToggleNiriControl { slot } => {
                if let Some(key) = self.physical_link(slot).map(handover_key) {
                    let key = key.to_owned();
                    if !self.handed_over_controllers.remove(&key) {
                        self.handed_over_controllers.insert(key);
                    }
                }
                Action::Idle
            }
            GamepadMessage::Expansion { slot, expansion } => {
                if let Some(session) = self.sessions.get_mut(&slot) {
                    session.expansion.update(expansion);
                }
                Action::Idle
            }
        }
    }

    fn receive_roster(&mut self, controllers: Vec<ConnectedGamepad>) {
        let connected = |slot: &usize| controllers.iter().any(|gamepad| gamepad.slot == *slot);
        self.sessions.retain(|slot, _| connected(slot));
        self.report_timings.retain(|slot, _| connected(slot));
        for gamepad in &controllers {
            self.sessions.entry(gamepad.slot).or_default();
        }
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
        if !releasing_push_to_talk && !self.acts(slot, bindings, window_claim) {
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

    fn tooltip_body(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
        clock: Clock,
    ) -> Option<TooltipBody> {
        Some(match target {
            GamepadTarget::Controller { slot } => self
                .face_reading(
                    self.controller(slot)?,
                    self.physical_link(slot)?,
                    bindings,
                    window_claim,
                )
                .tooltip(clock),
            GamepadTarget::Battery { slot } => ControllerBatteryReading {
                power: self.controller(slot)?.power,
            }
            .tooltip(clock),
            GamepadTarget::Link { slot } => self
                .link_reading(slot, self.physical_link(slot)?)
                .tooltip(clock),
            GamepadTarget::UsbCable { slot } => UsbCableReading {
                role: usb_cable_role(self.physical_link(slot)?.transport)?,
            }
            .tooltip(clock),
            GamepadTarget::NiriControl { slot } => {
                self.physical_link(slot)?;
                self.niri_control_tooltip(slot, bindings, window_claim)
            }
        })
    }

    fn niri_control_tooltip(
        &self,
        slot: usize,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
    ) -> TooltipBody {
        let (note_title, note_body) = match self.control(slot, bindings, window_claim) {
            CompositorControl::DrivingNiri => (
                "Driving niri",
                "Buttons and sticks focus and move niri windows. Turn this off when a game keeps \
                 losing its window to niri: the controller then belongs to the focused window \
                 until you turn it back on."
                    .to_owned(),
            ),
            CompositorControl::HandedOverByHand => (
                "Handed to the focused window",
                "Niri bindings stay off for this controller, whatever window has focus, until you \
                 turn this back on."
                    .to_owned(),
            ),
            CompositorControl::PausedForFocusedApp(app_id) => (
                "Paused while a game has focus",
                format!(
                    "{app_id} has focus, so niri bindings already pause on their own. Turning \
                     this off keeps them off after focus moves."
                ),
            ),
            CompositorControl::PausedForFullscreenApp(app_id) => (
                "Paused while a window fills the screen",
                format!(
                    "{app_id} fills the screen, so niri bindings pause even while a prompt has \
                     focus. Turning this off keeps them off after it leaves fullscreen."
                ),
            ),
        };
        TooltipBody::sections([Section::detailed_note(note_title, note_body)])
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'_>,
        clock: Clock,
    ) -> Option<Tooltip> {
        self.tooltip_body(target, bindings, window_claim, clock)
            .map(|body| Tooltip::sourced(body, DeliveryTiming::LivePush))
    }

    fn controller_group<'a>(
        &'a self,
        gamepad: &'a ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
        bindings: &'a GamepadBindingsConfig,
        window_claim: GamepadWindowClaim<'a>,
    ) -> Option<WidgetGroupContent<'a, GamepadMessage>> {
        let slot = gamepad.slot;
        let session = self.sessions.get(&slot)?;
        let battery_reading = ControllerBatteryReading {
            power: gamepad.power,
        };
        let battery_low = battery_reading.running_low();
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
                GroupItem::reading(battery_reading, GamepadTarget::Battery { slot })
                    .previewed_on_handle_if(battery_low)
            });
        let niri_control = GroupItem::toggle(
            niri_control_toggle(!self.handed_over(slot)),
            GamepadTarget::NiriControl { slot },
            GamepadMessage::ToggleNiriControl { slot },
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
        Some(Self::expandable_group(
            &session.expansion,
            move |expansion| GamepadMessage::Expansion { slot, expansion },
            [
                Some(face),
                Some(niri_control),
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
        self.controllers_in_player_order()
            .into_iter()
            .filter_map(|(gamepad, link)| {
                self.controller_group(gamepad, link, bindings, window_claim)
            })
            .map(WidgetGroupContent::sharing_title_row)
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

fn spawned(command: impl IntoIterator<Item = impl Into<String>>) -> niri_ipc::Action {
    niri_ipc::Action::Spawn {
        command: command.into_iter().map(Into::into).collect(),
    }
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
            Some(Action::Compositor(spawned(["wtype", "-k", key])))
        }
        (ButtonRole::Compositor(action), ButtonEdge::Pressed) => {
            Some(Action::Compositor(niri_request(action)))
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

fn niri_request(action: CompositorAction) -> niri_ipc::Action {
    match action {
        CompositorAction::FocusColumnLeft => niri_ipc::Action::FocusColumnLeft {},
        CompositorAction::FocusColumnRight => niri_ipc::Action::FocusColumnRight {},
        CompositorAction::FocusWindowUp => niri_ipc::Action::FocusWindowUp {},
        CompositorAction::FocusWindowDown => niri_ipc::Action::FocusWindowDown {},
        CompositorAction::FocusWorkspaceUp => niri_ipc::Action::FocusWorkspaceUp {},
        CompositorAction::FocusWorkspaceDown => niri_ipc::Action::FocusWorkspaceDown {},
        CompositorAction::FocusWindowPrevious => niri_ipc::Action::FocusWindowPrevious {},
        CompositorAction::MoveColumnLeft => niri_ipc::Action::MoveColumnLeft {},
        CompositorAction::MoveColumnRight => niri_ipc::Action::MoveColumnRight {},
        CompositorAction::MoveWindowToWorkspaceUp => {
            niri_ipc::Action::MoveWindowToWorkspaceUp { focus: true }
        }
        CompositorAction::MoveWindowToWorkspaceDown => {
            niri_ipc::Action::MoveWindowToWorkspaceDown { focus: true }
        }
        CompositorAction::ToggleOverview => niri_ipc::Action::ToggleOverview {},
        CompositorAction::MaximizeColumn => niri_ipc::Action::MaximizeColumn {},
        CompositorAction::CenterColumn => niri_ipc::Action::CenterColumn {},
        CompositorAction::CloseWindow => niri_ipc::Action::CloseWindow { id: None },
    }
}

#[cfg(test)]
mod tests {
    use sottomano_config::widget_groups::gamepad_bindings::{GamepadStick, StickDirection};
    use sottomano_sources::gamepad_controller_input::player_indicator_leds::LitPlayer;

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
        let Action::Compositor(niri_ipc::Action::Spawn { command }) = action else {
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
    fn flicking_the_left_stick_right_focuses_the_next_column() {
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
            Action::Compositor(niri_ipc::Action::FocusColumnRight {})
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
    fn controllers_drive_niri_unless_a_game_has_focus() {
        let bindings = GamepadBindingsConfig::default();
        let group = group_with_two_pads_and_a_virtual_one();
        assert_eq!(
            group.control(DUALSENSE, &bindings, GamepadWindowClaim::Focused("kitty")),
            CompositorControl::DrivingNiri
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
    fn a_handed_over_controller_stops_driving_niri_while_others_continue() {
        let mut group = group_with_two_pads_and_a_virtual_one();
        group.update(
            GamepadMessage::ToggleNiriControl { slot: DUALSENSE },
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
            GamepadMessage::ToggleNiriControl { slot: DUALSENSE },
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
            GamepadMessage::ToggleNiriControl { slot: DUALSENSE },
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
        assert!(
            group
                .stream_settings(&bindings, claim)
                .gate
                .silenced_by_window_claim
        );
    }

    #[test]
    fn an_unclaimed_desktop_leaves_controllers_driving_niri() {
        let bindings = GamepadBindingsConfig::default();
        let group = group_with_two_pads_and_a_virtual_one();
        assert_eq!(
            group.control(DUALSENSE, &bindings, GamepadWindowClaim::Unclaimed),
            CompositorControl::DrivingNiri
        );
    }

    #[test]
    fn focusing_a_game_silences_the_pointer_through_the_stream_settings() {
        let settings = GamepadGroup::default().stream_settings(
            &GamepadBindingsConfig::default(),
            GamepadWindowClaim::Focused("steam_app_1091500"),
        );
        assert!(settings.gate.silenced_by_window_claim);
    }
}
