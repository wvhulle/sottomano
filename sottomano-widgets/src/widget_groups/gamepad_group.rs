mod connected_controller_reading;
pub mod controller_button_labels;
mod diagram_anchor_mapping;

use std::collections::{BTreeMap, BTreeSet};

use iced::Subscription;
use sottomano_config::widget_groups::gamepad_bindings::{
    ButtonRole, CompositorAction, GamepadBindingsConfig, GamepadButton,
};
use sottomano_sources::gamepad_controller_input::{
    evdev_gamepad_devices::{ButtonEdge, ConnectedGamepad},
    gamepad_input_machine::{GamepadInput, GamepadStreamSettings, PointerGate},
    gamepad_input_stream::gamepad_inputs,
};

use self::connected_controller_reading::ConnectedControllerReading;
use crate::{
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        tooltip_body::{Freshness, Tooltip},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadTarget {
    Controller { slot: usize },
}

impl From<GamepadTarget> for HoverTarget {
    fn from(target: GamepadTarget) -> Self {
        HoverTarget::Gamepad(target)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ControllerOwner {
    #[default]
    Shell,
    HandedOver,
}

impl ControllerOwner {
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            ControllerOwner::Shell => ControllerOwner::HandedOver,
            ControllerOwner::HandedOver => ControllerOwner::Shell,
        }
    }

    #[must_use]
    pub const fn toggle_press_hint(self) -> &'static str {
        match self {
            ControllerOwner::Shell => "hand this controller to Steam and games",
            ControllerOwner::HandedOver => "take this controller back",
        }
    }
}

#[derive(Debug, Clone)]
pub enum GamepadMessage {
    Input(GamepadInput),
    ToggleOwner { slot: usize },
    Expansion(ExpansionMessage),
}

#[derive(Debug, Clone)]
pub enum Action {
    Idle,
    Compositor(niri_ipc::Action),
}

#[derive(Debug, Clone, Default)]
struct ControllerSession {
    held: BTreeSet<GamepadButton>,
    owner: ControllerOwner,
}

#[derive(Default)]
pub struct GamepadGroup {
    controllers: Vec<ConnectedGamepad>,
    sessions: BTreeMap<usize, ControllerSession>,
    expansion: GroupExpansion,
}

impl GamepadGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    fn owner(&self, slot: usize) -> ControllerOwner {
        self.sessions
            .get(&slot)
            .map_or(ControllerOwner::Shell, |session| session.owner)
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
                handed_over_slots: self
                    .sessions
                    .iter()
                    .filter(|(_, session)| session.owner == ControllerOwner::HandedOver)
                    .map(|(&slot, _)| slot)
                    .collect(),
            },
        }
    }

    pub fn subscription(
        &self,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Subscription<GamepadMessage> {
        Subscription::run_with(self.stream_settings(bindings, focused_app_id), |settings| {
            gamepad_inputs(settings.clone())
        })
        .map(GamepadMessage::Input)
    }

    fn acts(
        &self,
        slot: usize,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> bool {
        self.owner(slot) == ControllerOwner::Shell && bindings.acts_while_focused(focused_app_id)
    }

    pub fn update(
        &mut self,
        message: GamepadMessage,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Action {
        match message {
            GamepadMessage::Input(GamepadInput::Roster(controllers)) => {
                self.sessions
                    .retain(|slot, _| controllers.iter().any(|gamepad| gamepad.slot == *slot));
                self.controllers = controllers;
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::Button { slot, button, edge }) => {
                self.receive_button(slot, button, edge, bindings, focused_app_id)
            }
            GamepadMessage::Input(GamepadInput::StickFlick {
                slot,
                stick,
                direction,
            }) => bindings
                .navigation
                .flick_action(stick, direction)
                .filter(|_| self.acts(slot, bindings, focused_app_id))
                .map_or(Action::Idle, |action| {
                    Action::Compositor(niri_request(action))
                }),
            GamepadMessage::ToggleOwner { slot } => {
                let session = self.sessions.entry(slot).or_default();
                session.owner = session.owner.toggled();
                Action::Idle
            }
            GamepadMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
                Action::Idle
            }
        }
    }

    fn receive_button(
        &mut self,
        slot: usize,
        button: GamepadButton,
        edge: ButtonEdge,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Action {
        let session = self.sessions.entry(slot).or_default();
        match edge {
            ButtonEdge::Pressed => {
                session.held.insert(button);
                if bindings.completes_handover_chord(button, |held| session.held.contains(&held)) {
                    session.owner = session.owner.toggled();
                    return Action::Idle;
                }
            }
            ButtonEdge::Released => {
                session.held.remove(&button);
            }
        }
        let releasing_push_to_talk = edge == ButtonEdge::Released
            && bindings.role_of(button) == Some(ButtonRole::PushToTalk);
        if !releasing_push_to_talk && !self.acts(slot, bindings, focused_app_id) {
            return Action::Idle;
        }
        bindings
            .role_of(button)
            .and_then(|role| role_request(role, edge, bindings))
            .map_or(Action::Idle, Action::Compositor)
    }

    fn controller(&self, slot: usize) -> Option<&ConnectedGamepad> {
        self.controllers
            .iter()
            .find(|controller| controller.slot == slot)
    }

    fn player_number(&self, slot: usize) -> Option<usize> {
        (self.controllers.len() > 1)
            .then(|| {
                self.controllers
                    .iter()
                    .position(|controller| controller.slot == slot)
            })
            .flatten()
            .map(|index| index + 1)
    }

    fn reading<'a>(
        &self,
        gamepad: &'a ConnectedGamepad,
        bindings: &'a GamepadBindingsConfig,
    ) -> ConnectedControllerReading<'a> {
        ConnectedControllerReading {
            gamepad,
            bindings,
            owner: self.owner(gamepad.slot),
            player_number: self.player_number(gamepad.slot),
        }
    }

    #[must_use]
    pub fn tooltip_for(
        &self,
        target: GamepadTarget,
        bindings: &GamepadBindingsConfig,
        clock: Clock,
    ) -> Option<Tooltip> {
        let GamepadTarget::Controller { slot } = target;
        let reading = self.reading(self.controller(slot)?, bindings);
        Some(Tooltip::sourced(reading.tooltip(clock), Freshness::Live))
    }

    pub fn group_content<'a>(
        &'a self,
        bindings: &'a GamepadBindingsConfig,
    ) -> GroupContent<'a, GamepadMessage> {
        GroupContent::new(
            &self.expansion,
            GamepadMessage::Expansion,
            self.controllers.iter().map(move |gamepad| {
                GroupItem::reading(
                    self.reading(gamepad, bindings),
                    GamepadTarget::Controller { slot: gamepad.slot },
                )
                .on_press(
                    self.owner(gamepad.slot).toggle_press_hint(),
                    GamepadMessage::ToggleOwner { slot: gamepad.slot },
                )
            }),
        )
    }
}

fn spawned(command: impl IntoIterator<Item = impl Into<String>>) -> niri_ipc::Action {
    niri_ipc::Action::Spawn {
        command: command.into_iter().map(Into::into).collect(),
    }
}

fn role_request(
    role: ButtonRole<'_>,
    edge: ButtonEdge,
    bindings: &GamepadBindingsConfig,
) -> Option<niri_ipc::Action> {
    match (role, edge) {
        (ButtonRole::PushToTalk, ButtonEdge::Pressed) => {
            Some(spawned(bindings.push_to_talk.start.iter().cloned()))
        }
        (ButtonRole::PushToTalk, ButtonEdge::Released) => {
            Some(spawned(bindings.push_to_talk.stop.iter().cloned()))
        }
        (ButtonRole::TypedKey(key), ButtonEdge::Pressed) => Some(spawned(["wtype", "-k", key])),
        (ButtonRole::Compositor(action), ButtonEdge::Pressed) => Some(niri_request(action)),
        (ButtonRole::Command(command), ButtonEdge::Pressed) => {
            Some(spawned(command.iter().cloned()))
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
        assert_eq!(spawned_command(started), ["voxtype", "record", "start"]);
        let stopped = release(&mut group, DUALSENSE, GamepadButton::LeftTrigger);
        assert_eq!(spawned_command(stopped), ["voxtype", "record", "stop"]);
    }

    #[test]
    fn the_handover_chord_hands_over_only_the_controller_that_pressed_it() {
        let mut group = GamepadGroup::default();
        press(&mut group, DUALSENSE, GamepadButton::Back);
        assert!(matches!(
            press(&mut group, DUALSENSE, GamepadButton::Start),
            Action::Idle
        ));
        assert_eq!(group.owner(DUALSENSE), ControllerOwner::HandedOver);
        assert!(matches!(
            press(&mut group, DUALSENSE, GamepadButton::North),
            Action::Idle
        ));
        assert!(matches!(
            press(&mut group, SECOND_PAD, GamepadButton::North),
            Action::Compositor(niri_ipc::Action::MaximizeColumn {})
        ));
        assert_eq!(
            group
                .stream_settings(&GamepadBindingsConfig::default(), Some("kitty"))
                .gate
                .handed_over_slots,
            BTreeSet::from([DUALSENSE])
        );
    }

    #[test]
    fn focusing_a_game_silences_the_pointer_through_the_stream_settings() {
        let settings = GamepadGroup::default()
            .stream_settings(&GamepadBindingsConfig::default(), Some("steam_app_1091500"));
        assert!(settings.gate.silenced_by_focus);
    }
}
