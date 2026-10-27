mod connected_controller_reading;
mod controller_battery_reading;
pub mod controller_button_labels;
mod controller_link_reading;
mod controller_transport_reading;
mod diagram_anchor_mapping;
mod player_number_reading;

use std::collections::BTreeMap;

use iced::Subscription;
use notulist_client::DictationCommand;
use sottomano_config::widget_groups::gamepad_bindings::{
    ButtonRole, CompositorAction, GamepadBindingsConfig, GamepadButton,
};
use sottomano_sources::{
    gamepad_controller_input::{
        accelerometer_report_rate::ReportRate,
        evdev_gamepad_devices::{
            ButtonEdge, ConnectedGamepad, ControllerIdentity, GamepadOrigin, GamepadPower,
            GamepadTransport, PhysicalGamepadLink,
        },
        gamepad_input_machine::{GamepadInput, GamepadStreamSettings, PointerGate},
        gamepad_input_stream::gamepad_inputs,
        player_indicator_leds::PlayerIndicator,
    },
    systemd_units_journal::journal_device_kernel_errors::{
        DeviceKernelError, device_kernel_error_windows,
    },
};

use self::{
    connected_controller_reading::ConnectedControllerReading,
    controller_battery_reading::ControllerBatteryReading,
    controller_link_reading::{ControllerLinkReading, MeasuredReportRate},
    controller_transport_reading::ControllerTransportReading,
    player_number_reading::PlayerNumberReading,
};
use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        tooltip_body::{DeliveryTiming, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadTarget {
    Controller { slot: usize },
    Player { slot: usize },
    Battery { slot: usize },
    Link { slot: usize },
    Transport { slot: usize },
}

impl From<GamepadTarget> for HoverTarget {
    fn from(target: GamepadTarget) -> Self {
        HoverTarget::Gamepad(target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorControl<'a> {
    DrivingNiri,
    PausedForFocusedApp(&'a str),
}

#[derive(Debug, Clone)]
pub enum GamepadMessage {
    Input(GamepadInput),
    KernelErrors(Vec<DeviceKernelError>),
    DisconnectBluetooth {
        slot: usize,
    },
    Expansion {
        slot: usize,
        expansion: ExpansionMessage,
    },
}

#[derive(Debug, Clone)]
pub enum Action {
    Idle,
    Compositor(niri_ipc::Action),
    Dictation(DictationCommand),
}

#[derive(Default)]
struct ControllerSession {
    expansion: GroupExpansion,
}

#[derive(Default)]
pub struct GamepadGroup {
    controllers: Vec<ConnectedGamepad>,
    sessions: BTreeMap<usize, ControllerSession>,
    report_rates: BTreeMap<usize, ReportRate>,
    kernel_errors: Vec<DeviceKernelError>,
}

impl GamepadGroup {
    pub fn collapse(&mut self) {
        for session in self.sessions.values_mut() {
            session.expansion.collapse();
        }
    }

    fn control<'a>(
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&'a str>,
    ) -> CompositorControl<'a> {
        focused_app_id
            .filter(|app_id| !bindings.acts_while_focused(Some(app_id)))
            .map_or(
                CompositorControl::DrivingNiri,
                CompositorControl::PausedForFocusedApp,
            )
    }

    #[must_use]
    pub fn stream_settings(
        &self,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> GamepadStreamSettings {
        GamepadStreamSettings {
            steering: bindings.pointer_steering(),
            gate: PointerGate {
                silenced_by_focus: !bindings.acts_while_focused(focused_app_id),
            },
        }
    }

    pub fn subscription(
        &self,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Subscription<GamepadMessage> {
        Subscription::batch([
            Subscription::run_with(self.stream_settings(bindings, focused_app_id), |settings| {
                gamepad_inputs(settings.clone())
            })
            .map(GamepadMessage::Input),
            Subscription::run(device_kernel_error_windows).map(GamepadMessage::KernelErrors),
        ])
    }

    fn acts(bindings: &GamepadBindingsConfig, focused_app_id: Option<&str>) -> bool {
        Self::control(bindings, focused_app_id) == CompositorControl::DrivingNiri
    }

    pub fn update(
        &mut self,
        message: GamepadMessage,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Action {
        match message {
            GamepadMessage::Input(GamepadInput::Roster(controllers)) => {
                self.receive_roster(controllers);
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::ReportRate { slot, rate }) => {
                self.report_rates.insert(slot, rate);
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::Button { button, edge, .. }) => {
                Self::receive_button(button, edge, bindings, focused_app_id)
            }
            GamepadMessage::Input(GamepadInput::StickFlick {
                stick, direction, ..
            }) => bindings
                .navigation
                .flick_action(stick, direction)
                .filter(|_| Self::acts(bindings, focused_app_id))
                .map_or(Action::Idle, |action| {
                    Action::Compositor(niri_request(action))
                }),
            GamepadMessage::KernelErrors(kernel_errors) => {
                self.kernel_errors = kernel_errors;
                Action::Idle
            }
            GamepadMessage::DisconnectBluetooth { slot } => {
                self.bluetooth_address(slot)
                    .map_or(Action::Idle, |hardware_address| {
                        Action::Compositor(spawned([
                            "bluetoothctl",
                            "disconnect",
                            hardware_address,
                        ]))
                    })
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
        self.report_rates.retain(|slot, _| connected(slot));
        for gamepad in &controllers {
            self.sessions.entry(gamepad.slot).or_default();
        }
        self.controllers = controllers;
    }

    fn receive_button(
        button: GamepadButton,
        edge: ButtonEdge,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Action {
        let releasing_push_to_talk = edge == ButtonEdge::Released
            && bindings.role_of(button) == Some(ButtonRole::PushToTalk);
        if !releasing_push_to_talk && !Self::acts(bindings, focused_app_id) {
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

    fn face_reading<'a>(
        gamepad: &'a ConnectedGamepad,
        bindings: &'a GamepadBindingsConfig,
        focused_app_id: Option<&'a str>,
    ) -> ConnectedControllerReading<'a> {
        ConnectedControllerReading {
            gamepad,
            bindings,
            control: Self::control(bindings, focused_app_id),
        }
    }

    fn link_reading<'a>(
        &'a self,
        slot: usize,
        link: &'a PhysicalGamepadLink,
    ) -> ControllerLinkReading<'a> {
        ControllerLinkReading {
            transport: &link.transport,
            report_rate: self
                .report_rates
                .get(&slot)
                .copied()
                .map_or(MeasuredReportRate::Pending, MeasuredReportRate::Measured),
            kernel_errors: self
                .kernel_errors
                .iter()
                .filter(|error| error.kernel_device == link.kernel_device)
                .collect(),
        }
    }

    fn tooltip_body(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
        clock: Clock,
    ) -> Option<TooltipBody> {
        Some(match target {
            GamepadTarget::Controller { slot } => {
                Self::face_reading(self.controller(slot)?, bindings, focused_app_id).tooltip(clock)
            }
            GamepadTarget::Player { slot } => match &self.physical_link(slot)?.player_indicator {
                PlayerIndicator::Lit(lit_player) => {
                    PlayerNumberReading { lit_player }.tooltip(clock)
                }
                PlayerIndicator::Unlit => return None,
            },
            GamepadTarget::Battery { slot } => ControllerBatteryReading {
                power: self.controller(slot)?.power,
            }
            .tooltip(clock),
            GamepadTarget::Link { slot } => self
                .link_reading(slot, self.physical_link(slot)?)
                .tooltip(clock),
            GamepadTarget::Transport { slot } => {
                let link = self.physical_link(slot)?;
                ControllerTransportReading {
                    transport: link.transport,
                    identity: &link.identity,
                }
                .tooltip(clock)
            }
        })
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
        clock: Clock,
    ) -> Option<Tooltip> {
        self.tooltip_body(target, bindings, focused_app_id, clock)
            .map(|body| Tooltip::sourced(body, DeliveryTiming::LivePush))
    }

    fn controller_group<'a>(
        &'a self,
        gamepad: &'a ConnectedGamepad,
        link: &'a PhysicalGamepadLink,
        bindings: &'a GamepadBindingsConfig,
        focused_app_id: Option<&'a str>,
    ) -> Option<GroupContent<'a, GamepadMessage>> {
        let slot = gamepad.slot;
        let session = self.sessions.get(&slot)?;
        let battery_reading = ControllerBatteryReading {
            power: gamepad.power,
        };
        let battery_low = battery_reading.running_low();
        let face = GroupItem::reading(
            Self::face_reading(gamepad, bindings, focused_app_id),
            GamepadTarget::Controller { slot },
        );
        let player = match &link.player_indicator {
            PlayerIndicator::Lit(lit_player) => Some(GroupItem::reading(
                PlayerNumberReading { lit_player },
                GamepadTarget::Player { slot },
            )),
            PlayerIndicator::Unlit => None,
        };
        let connection =
            GroupItem::reading(self.link_reading(slot, link), GamepadTarget::Link { slot });
        let battery = (gamepad.power != GamepadPower::NoBattery).then(|| {
            GroupItem::reading(battery_reading, GamepadTarget::Battery { slot })
                .previewed_on_handle_if(battery_low)
        });
        let transport_reading = ControllerTransportReading {
            transport: link.transport,
            identity: &link.identity,
        };
        let cable_ignored = transport_reading.cable_ignored();
        let transport = GroupItem::reading(transport_reading, GamepadTarget::Transport { slot });
        let transport = if cable_ignored {
            transport.on_press(
                "disconnect Bluetooth so the cable can take over",
                GamepadMessage::DisconnectBluetooth { slot },
            )
        } else {
            transport
        };
        Some(GroupContent::new(
            &session.expansion,
            move |expansion| GamepadMessage::Expansion { slot, expansion },
            [
                Some(face),
                player,
                Some(connection),
                battery,
                Some(transport),
            ]
            .into_iter()
            .flatten(),
        ))
    }

    #[must_use]
    pub fn group_contents<'a>(
        &'a self,
        bindings: &'a GamepadBindingsConfig,
        focused_app_id: Option<&'a str>,
    ) -> Vec<GroupContent<'a, GamepadMessage>> {
        self.controllers_in_player_order()
            .into_iter()
            .filter_map(|(gamepad, link)| {
                self.controller_group(gamepad, link, bindings, focused_app_id)
            })
            .collect()
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
    use sottomano_sources::gamepad_controller_input::{
        evdev_gamepad_devices::UsbCableState, player_indicator_leds::LitPlayer,
    };

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
            Some("kitty"),
        )
    }

    fn release(group: &mut GamepadGroup, slot: usize, released: GamepadButton) -> Action {
        group.update(
            button(slot, released, ButtonEdge::Released),
            &GamepadBindingsConfig::default(),
            Some("kitty"),
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
            Some("kitty"),
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
            Some("steam_app_1091500"),
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
            Some("kitty"),
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
                .group_contents(&GamepadBindingsConfig::default(), Some("kitty"))
                .len(),
            2
        );
    }

    #[test]
    fn controllers_drive_niri_unless_a_game_has_focus() {
        let bindings = GamepadBindingsConfig::default();
        assert_eq!(
            GamepadGroup::control(&bindings, Some("kitty")),
            CompositorControl::DrivingNiri
        );
        assert_eq!(
            GamepadGroup::control(&bindings, Some("steam_app_1091500")),
            CompositorControl::PausedForFocusedApp("steam_app_1091500")
        );
    }

    #[test]
    fn focusing_a_game_silences_the_pointer_through_the_stream_settings() {
        let settings = GamepadGroup::default()
            .stream_settings(&GamepadBindingsConfig::default(), Some("steam_app_1091500"));
        assert!(settings.gate.silenced_by_focus);
    }
}
