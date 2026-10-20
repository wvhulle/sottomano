mod connected_controller_reading;
pub mod controller_button_labels;

use std::collections::BTreeSet;

use iced::Subscription;
use sottomano_config::widget_groups::gamepad_bindings::{
    ButtonBinding, CompositorAction, GamepadBindingsConfig, GamepadButton,
};
use sottomano_sources::gamepad_controller_input::{
    gilrs_gamepad_events::{ButtonEdge, ConnectedGamepad, GamepadInput, gamepad_inputs},
    wlr_virtual_pointer_motion::PointerGate,
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
            ControllerOwner::Shell => "hand the controller to Steam and games",
            ControllerOwner::HandedOver => "take the controller back",
        }
    }
}

#[derive(Debug, Clone)]
pub enum GamepadMessage {
    Input(GamepadInput),
    ToggleOwner,
    Expansion(ExpansionMessage),
}

#[derive(Debug, Clone)]
pub enum Action {
    Idle,
    Compositor(niri_ipc::Action),
}

#[derive(Default)]
pub struct GamepadGroup {
    controllers: Vec<ConnectedGamepad>,
    held: BTreeSet<GamepadButton>,
    owner: ControllerOwner,
    pointer_gate: Option<PointerGate>,
    expansion: GroupExpansion,
}

impl GamepadGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(bindings: &GamepadBindingsConfig) -> Subscription<GamepadMessage> {
        Subscription::run_with(bindings.pointer_steering(), |steering| {
            gamepad_inputs(steering.clone())
        })
        .map(GamepadMessage::Input)
    }

    fn acts(&self, bindings: &GamepadBindingsConfig, focused_app_id: Option<&str>) -> bool {
        self.owner == ControllerOwner::Shell && bindings.acts_while_focused(focused_app_id)
    }

    pub fn follow_focus(&self, bindings: &GamepadBindingsConfig, focused_app_id: Option<&str>) {
        if let Some(gate) = &self.pointer_gate {
            gate.allow(self.acts(bindings, focused_app_id));
        }
    }

    pub fn update(
        &mut self,
        message: GamepadMessage,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Action {
        let action = match message {
            GamepadMessage::Input(GamepadInput::PointerGate(gate)) => {
                self.pointer_gate = Some(gate);
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::Roster(controllers)) => {
                self.controllers = controllers;
                self.held.clear();
                Action::Idle
            }
            GamepadMessage::Input(GamepadInput::Button { button, edge, .. }) => {
                self.receive_button(button, edge, bindings, focused_app_id)
            }
            GamepadMessage::Input(GamepadInput::StickFlick {
                stick, direction, ..
            }) => match bindings.sticks.of(stick).flick_binding(direction) {
                Some(binding) if self.acts(bindings, focused_app_id) => binding_action(binding),
                _ => Action::Idle,
            },
            GamepadMessage::ToggleOwner => {
                self.owner = self.owner.toggled();
                Action::Idle
            }
            GamepadMessage::Expansion(expansion) => {
                self.expansion.update(expansion);
                Action::Idle
            }
        };
        self.follow_focus(bindings, focused_app_id);
        action
    }

    fn receive_button(
        &mut self,
        button: GamepadButton,
        edge: ButtonEdge,
        bindings: &GamepadBindingsConfig,
        focused_app_id: Option<&str>,
    ) -> Action {
        let bound = bindings.buttons.get(&button);
        let binding = match edge {
            ButtonEdge::Pressed => {
                self.held.insert(button);
                if bindings.completes_handover_chord(button, |held| self.held.contains(&held)) {
                    self.owner = self.owner.toggled();
                    return Action::Idle;
                }
                bound.and_then(|bound| bound.press.as_ref())
            }
            ButtonEdge::Released => {
                self.held.remove(&button);
                bound.and_then(|bound| bound.release.as_ref())
            }
        };
        if !self.acts(bindings, focused_app_id) {
            return Action::Idle;
        }
        binding.map_or(Action::Idle, binding_action)
    }

    fn controller(&self, slot: usize) -> Option<&ConnectedGamepad> {
        self.controllers
            .iter()
            .find(|controller| controller.slot == slot)
    }

    fn reading<'a>(
        &self,
        gamepad: &'a ConnectedGamepad,
        bindings: &'a GamepadBindingsConfig,
    ) -> ConnectedControllerReading<'a> {
        ConnectedControllerReading {
            gamepad,
            bindings,
            owner: self.owner,
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
                .on_press(self.owner.toggle_press_hint(), GamepadMessage::ToggleOwner)
            }),
        )
    }
}

fn binding_action(binding: &ButtonBinding) -> Action {
    match binding {
        ButtonBinding::Compositor(action) => Action::Compositor(niri_request(*action)),
        ButtonBinding::Command(command) => Action::Compositor(niri_ipc::Action::Spawn {
            command: command.clone(),
        }),
        ButtonBinding::PointerButton(_) => Action::Idle,
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

    fn button(button: GamepadButton, edge: ButtonEdge) -> GamepadMessage {
        GamepadMessage::Input(GamepadInput::Button {
            slot: 0,
            button,
            edge,
        })
    }

    fn press(group: &mut GamepadGroup, pressed: GamepadButton) -> Action {
        group.update(
            button(pressed, ButtonEdge::Pressed),
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
        let action = press(&mut GamepadGroup::default(), GamepadButton::DpadLeft);
        assert_eq!(spawned_command(action), ["wtype", "-k", "Left"]);
    }

    #[test]
    fn flicking_the_left_stick_right_focuses_the_next_column() {
        let action = GamepadGroup::default().update(
            GamepadMessage::Input(GamepadInput::StickFlick {
                slot: 0,
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
    fn focusing_a_game_closes_the_pointer_gate() {
        let mut group = GamepadGroup::default();
        let gate = PointerGate::default();
        let config = GamepadBindingsConfig::default();
        group.update(
            GamepadMessage::Input(GamepadInput::PointerGate(gate.clone())),
            &config,
            Some("kitty"),
        );
        assert!(gate.is_allowed());
        group.follow_focus(&config, Some("steam_app_1091500"));
        assert!(!gate.is_allowed());
    }

    #[test]
    fn a_focused_game_keeps_its_controller_input() {
        let action = GamepadGroup::default().update(
            button(GamepadButton::DpadLeft, ButtonEdge::Pressed),
            &GamepadBindingsConfig::default(),
            Some("steam_app_1091500"),
        );
        assert!(matches!(action, Action::Idle));
    }

    #[test]
    fn releasing_the_left_trigger_stops_dictation() {
        let action = GamepadGroup::default().update(
            button(GamepadButton::LeftTrigger, ButtonEdge::Released),
            &GamepadBindingsConfig::default(),
            None,
        );
        assert_eq!(spawned_command(action), ["voxtype", "record", "stop"]);
    }

    #[test]
    fn the_handover_chord_silences_bindings_until_pressed_again() {
        let mut group = GamepadGroup::default();
        let chord = |group: &mut GamepadGroup| {
            press(group, GamepadButton::Select);
            let action = press(group, GamepadButton::Start);
            let config = GamepadBindingsConfig::default();
            group.update(
                button(GamepadButton::Start, ButtonEdge::Released),
                &config,
                None,
            );
            group.update(
                button(GamepadButton::Select, ButtonEdge::Released),
                &config,
                None,
            );
            action
        };
        assert!(matches!(chord(&mut group), Action::Idle));
        assert_eq!(group.owner, ControllerOwner::HandedOver);
        assert!(matches!(
            press(&mut group, GamepadButton::DpadLeft),
            Action::Idle
        ));
        chord(&mut group);
        assert_eq!(group.owner, ControllerOwner::Shell);
        assert!(matches!(
            press(&mut group, GamepadButton::DpadLeft),
            Action::Compositor(_)
        ));
    }
}
