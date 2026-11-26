use std::collections::{BTreeMap, BTreeSet};

pub use gamepad_input_vocabulary::{
    CompositorAction, GamepadButton, GamepadStick, PointerButton, StickDirection,
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod gamepad_bindings_tests;
mod gamepad_input_vocabulary;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StickNavigationConfig {
    pub stick: Option<GamepadStick>,
    pub up: Option<CompositorAction>,
    pub down: Option<CompositorAction>,
    pub left: Option<CompositorAction>,
    pub right: Option<CompositorAction>,
}

impl StickNavigationConfig {
    #[must_use]
    pub const fn action(&self, direction: StickDirection) -> Option<CompositorAction> {
        match direction {
            StickDirection::Up => self.up,
            StickDirection::Down => self.down,
            StickDirection::Left => self.left,
            StickDirection::Right => self.right,
        }
    }

    #[must_use]
    pub fn flick_action(
        &self,
        stick: GamepadStick,
        direction: StickDirection,
    ) -> Option<CompositorAction> {
        (self.stick == Some(stick))
            .then(|| self.action(direction))
            .flatten()
    }
}

impl Default for StickNavigationConfig {
    fn default() -> Self {
        Self {
            stick: Some(GamepadStick::Left),
            up: Some(CompositorAction::FocusWorkspaceUp),
            down: Some(CompositorAction::FocusWorkspaceDown),
            left: Some(CompositorAction::FocusLeft),
            right: Some(CompositorAction::FocusRight),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PointerSteeringConfig {
    pub stick: Option<GamepadStick>,
    pub pixels_per_second: u32,
    pub left_button: Option<GamepadButton>,
    pub right_button: Option<GamepadButton>,
    pub middle_button: Option<GamepadButton>,
}

impl PointerSteeringConfig {
    pub fn claimed_buttons(&self) -> impl Iterator<Item = ClaimedPointerButton> {
        [
            (PointerButton::Left, self.left_button),
            (PointerButton::Right, self.right_button),
            (PointerButton::Middle, self.middle_button),
        ]
        .into_iter()
        .filter_map(|(pointer_button, button)| {
            Some(ClaimedPointerButton {
                button: button?,
                pointer_button,
            })
        })
    }
}

impl Default for PointerSteeringConfig {
    fn default() -> Self {
        Self {
            stick: Some(GamepadStick::Right),
            pixels_per_second: 1600,
            left_button: Some(GamepadButton::RightTrigger),
            right_button: Some(GamepadButton::RightStick),
            middle_button: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimedPointerButton {
    pub button: GamepadButton,
    pub pointer_button: PointerButton,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PushToTalkConfig {
    pub button: Option<GamepadButton>,
}

impl Default for PushToTalkConfig {
    fn default() -> Self {
        Self {
            button: Some(GamepadButton::LeftTrigger),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointerStickSettings {
    pub stick: GamepadStick,
    pub pixels_per_second: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PointerStickBinding {
    #[default]
    Unbound,
    Bound(PointerStickSettings),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PointerSteeringSettings {
    pub stick: PointerStickBinding,
    pub buttons: BTreeMap<GamepadButton, PointerButton>,
}

impl PointerSteeringSettings {
    #[must_use]
    pub fn virtual_pointer_need(&self) -> VirtualPointerNeed {
        if self.stick != PointerStickBinding::Unbound || !self.buttons.is_empty() {
            VirtualPointerNeed::Needed
        } else {
            VirtualPointerNeed::Unneeded
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonRole<'bindings> {
    PushToTalk,
    PointerButton(PointerButton),
    Compositor(CompositorAction),
    TypedKey(&'bindings str),
    Command(&'bindings [String]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimedButton<'bindings> {
    pub button: GamepadButton,
    pub role: ButtonRole<'bindings>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GamepadBindingsConfig {
    pub navigation: StickNavigationConfig,
    pub pointer: PointerSteeringConfig,
    pub typed_keys: BTreeMap<GamepadButton, String>,
    pub compositor_actions: BTreeMap<GamepadButton, CompositorAction>,
    pub commands: BTreeMap<GamepadButton, Vec<String>>,
    pub push_to_talk: PushToTalkConfig,
}

impl GamepadBindingsConfig {
    pub fn claimed_buttons(&self) -> impl Iterator<Item = ClaimedButton<'_>> {
        let push_to_talk = self.push_to_talk.button.map(|button| ClaimedButton {
            button,
            role: ButtonRole::PushToTalk,
        });
        let pointer = self.pointer.claimed_buttons().map(|claimed| ClaimedButton {
            button: claimed.button,
            role: ButtonRole::PointerButton(claimed.pointer_button),
        });
        let compositor = self
            .compositor_actions
            .iter()
            .map(|(&button, &action)| ClaimedButton {
                button,
                role: ButtonRole::Compositor(action),
            });
        let typed_keys = self.typed_keys.iter().map(|(&button, key)| ClaimedButton {
            button,
            role: ButtonRole::TypedKey(key),
        });
        let commands = self
            .commands
            .iter()
            .map(|(&button, command)| ClaimedButton {
                button,
                role: ButtonRole::Command(command),
            });
        push_to_talk
            .into_iter()
            .chain(pointer)
            .chain(compositor)
            .chain(typed_keys)
            .chain(commands)
    }

    #[must_use]
    pub fn claimed_role(&self, button: GamepadButton) -> Option<ButtonRole<'_>> {
        self.claimed_buttons()
            .find(|claimed| claimed.button == button)
            .map(|claimed| claimed.role)
    }

    #[must_use]
    pub fn conflicting_buttons(&self) -> BTreeSet<GamepadButton> {
        let mut seen = BTreeSet::new();
        self.claimed_buttons()
            .filter(|claimed| !seen.insert(claimed.button))
            .map(|claimed| claimed.button)
            .collect()
    }

    #[must_use]
    pub fn stick_sharing(&self) -> StickSharing {
        if self.navigation.stick.is_some() && self.navigation.stick == self.pointer.stick {
            StickSharing::Shared
        } else {
            StickSharing::Separate
        }
    }

    #[must_use]
    pub fn pointer_steering(&self) -> PointerSteeringSettings {
        PointerSteeringSettings {
            stick: match self.pointer.stick {
                Some(stick) => PointerStickBinding::Bound(PointerStickSettings {
                    stick,
                    pixels_per_second: self.pointer.pixels_per_second,
                }),
                None => PointerStickBinding::Unbound,
            },
            buttons: self
                .claimed_buttons()
                .filter_map(|claimed| match claimed.role {
                    ButtonRole::PointerButton(pointer_button)
                        if self.claimed_role(claimed.button) == Some(claimed.role) =>
                    {
                        Some((claimed.button, pointer_button))
                    }
                    _ => None,
                })
                .collect(),
        }
    }
}

impl Default for GamepadBindingsConfig {
    fn default() -> Self {
        Self {
            navigation: StickNavigationConfig::default(),
            pointer: PointerSteeringConfig::default(),
            typed_keys: BTreeMap::from([
                (GamepadButton::DpadLeft, "Left".to_owned()),
                (GamepadButton::DpadRight, "Right".to_owned()),
                (GamepadButton::DpadUp, "Up".to_owned()),
                (GamepadButton::DpadDown, "Down".to_owned()),
                (GamepadButton::South, "Return".to_owned()),
                (GamepadButton::East, "BackSpace".to_owned()),
                (GamepadButton::North, "shift+Tab".to_owned()),
                (GamepadButton::West, "Escape".to_owned()),
                (GamepadButton::Back, "Tab".to_owned()),
            ]),
            compositor_actions: BTreeMap::from([
                (GamepadButton::LeftShoulder, CompositorAction::MoveLeft),
                (GamepadButton::RightShoulder, CompositorAction::MoveRight),
                (GamepadButton::Start, CompositorAction::Maximize),
                (
                    GamepadButton::LeftStick,
                    CompositorAction::FocusPreviousWindow,
                ),
                (GamepadButton::Guide, CompositorAction::ToggleOverview),
            ]),
            commands: BTreeMap::new(),
            push_to_talk: PushToTalkConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualPointerNeed {
    Needed,
    Unneeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickSharing {
    Shared,
    Separate,
}
