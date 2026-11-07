use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamepadButton {
    #[serde(alias = "cross", alias = "a")]
    South,
    #[serde(alias = "circle", alias = "b")]
    East,
    #[serde(alias = "triangle", alias = "y")]
    North,
    #[serde(alias = "square", alias = "x")]
    West,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    #[serde(alias = "l1", alias = "lb")]
    LeftShoulder,
    #[serde(alias = "r1", alias = "rb")]
    RightShoulder,
    #[serde(alias = "l2", alias = "lt")]
    LeftTrigger,
    #[serde(alias = "r2", alias = "rt")]
    RightTrigger,
    #[serde(alias = "create", alias = "view")]
    Back,
    #[serde(alias = "options", alias = "menu")]
    Start,
    #[serde(alias = "ps", alias = "xbox")]
    Guide,
    #[serde(alias = "l3", alias = "ls")]
    LeftStick,
    #[serde(alias = "r3", alias = "rs")]
    RightStick,
}

impl GamepadButton {
    #[must_use]
    pub const fn position_name(self) -> &'static str {
        match self {
            GamepadButton::South => "south face button",
            GamepadButton::East => "east face button",
            GamepadButton::North => "north face button",
            GamepadButton::West => "west face button",
            GamepadButton::DpadUp => "D-pad up",
            GamepadButton::DpadDown => "D-pad down",
            GamepadButton::DpadLeft => "D-pad left",
            GamepadButton::DpadRight => "D-pad right",
            GamepadButton::LeftShoulder => "left shoulder",
            GamepadButton::RightShoulder => "right shoulder",
            GamepadButton::LeftTrigger => "left trigger",
            GamepadButton::RightTrigger => "right trigger",
            GamepadButton::Back => "back",
            GamepadButton::Start => "start",
            GamepadButton::Guide => "guide",
            GamepadButton::LeftStick => "left stick press",
            GamepadButton::RightStick => "right stick press",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositorAction {
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    FocusWorkspaceUp,
    FocusWorkspaceDown,
    FocusPreviousWindow,
    MoveLeft,
    MoveRight,
    MoveWindowToWorkspaceUp,
    MoveWindowToWorkspaceDown,
    ToggleOverview,
    Maximize,
    Center,
    CloseWindow,
}

impl CompositorAction {
    #[must_use]
    pub const fn press_hint(self) -> &'static str {
        match self {
            CompositorAction::FocusLeft => "focus the window to the left",
            CompositorAction::FocusRight => "focus the window to the right",
            CompositorAction::FocusUp => "focus the window above",
            CompositorAction::FocusDown => "focus the window below",
            CompositorAction::FocusWorkspaceUp => "focus the workspace above",
            CompositorAction::FocusWorkspaceDown => "focus the workspace below",
            CompositorAction::FocusPreviousWindow => "focus the previous window",
            CompositorAction::MoveLeft => "move the window left",
            CompositorAction::MoveRight => "move the window right",
            CompositorAction::MoveWindowToWorkspaceUp => "move the window a workspace up",
            CompositorAction::MoveWindowToWorkspaceDown => "move the window a workspace down",
            CompositorAction::ToggleOverview => "toggle the overview",
            CompositorAction::Maximize => "maximize the window",
            CompositorAction::Center => "center the window",
            CompositorAction::CloseWindow => "close the window",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            CompositorAction::FocusLeft => "focus ←",
            CompositorAction::FocusRight => "focus →",
            CompositorAction::FocusUp => "focus ↑",
            CompositorAction::FocusDown => "focus ↓",
            CompositorAction::FocusWorkspaceUp => "workspace ↑",
            CompositorAction::FocusWorkspaceDown => "workspace ↓",
            CompositorAction::FocusPreviousWindow => "previous window",
            CompositorAction::MoveLeft => "move ←",
            CompositorAction::MoveRight => "move →",
            CompositorAction::MoveWindowToWorkspaceUp => "move to workspace ↑",
            CompositorAction::MoveWindowToWorkspaceDown => "move to workspace ↓",
            CompositorAction::ToggleOverview => "overview",
            CompositorAction::Maximize => "maximize",
            CompositorAction::Center => "center",
            CompositorAction::CloseWindow => "close window",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

impl PointerButton {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            PointerButton::Left => "Left mouse button",
            PointerButton::Right => "Right mouse button",
            PointerButton::Middle => "Middle mouse button",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            PointerButton::Left => "click",
            PointerButton::Right => "right click",
            PointerButton::Middle => "middle click",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamepadStick {
    Left,
    Right,
}

impl GamepadStick {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            GamepadStick::Left => "Left stick",
            GamepadStick::Right => "Right stick",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StickDirection {
    Up,
    Down,
    Left,
    Right,
}

impl StickDirection {
    pub const ALL: [StickDirection; 4] = [
        StickDirection::Up,
        StickDirection::Down,
        StickDirection::Left,
        StickDirection::Right,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            StickDirection::Up => "Up",
            StickDirection::Down => "Down",
            StickDirection::Left => "Left",
            StickDirection::Right => "Right",
        }
    }
}

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PointerSteeringSettings {
    pub stick: Option<PointerStickSettings>,
    pub buttons: BTreeMap<GamepadButton, PointerButton>,
}

impl PointerSteeringSettings {
    #[must_use]
    pub fn virtual_pointer_need(&self) -> VirtualPointerNeed {
        if self.stick.is_some() || !self.buttons.is_empty() {
            VirtualPointerNeed::Needed
        } else {
            VirtualPointerNeed::Unneeded
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonRole<'a> {
    PushToTalk,
    PointerButton(PointerButton),
    Compositor(CompositorAction),
    TypedKey(&'a str),
    Command(&'a [String]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimedButton<'a> {
    pub button: GamepadButton,
    pub role: ButtonRole<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GamepadBindingsConfig {
    pub passive_app_id_prefixes: Vec<String>,
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
    pub fn role_of(&self, button: GamepadButton) -> Option<ButtonRole<'_>> {
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
            stick: self.pointer.stick.map(|stick| PointerStickSettings {
                stick,
                pixels_per_second: self.pointer.pixels_per_second,
            }),
            buttons: self
                .claimed_buttons()
                .filter_map(|claimed| match claimed.role {
                    ButtonRole::PointerButton(pointer_button)
                        if self.role_of(claimed.button) == Some(claimed.role) =>
                    {
                        Some((claimed.button, pointer_button))
                    }
                    _ => None,
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn focus_behaviour(&self, app_id: Option<&str>) -> FocusBehaviour {
        let passive = app_id.is_some_and(|focused_app_id| {
            self.passive_app_id_prefixes
                .iter()
                .any(|prefix| focused_app_id.starts_with(prefix.as_str()))
        });
        if passive {
            FocusBehaviour::Passive
        } else {
            FocusBehaviour::Active
        }
    }
}

impl Default for GamepadBindingsConfig {
    fn default() -> Self {
        Self {
            passive_app_id_prefixes: ["steam", "gamescope"].map(String::from).to_vec(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusBehaviour {
    Active,
    Passive,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings_survive_a_toml_round_trip() {
        let defaults = GamepadBindingsConfig::default();
        let printed = toml::to_string(&defaults).unwrap();
        assert_eq!(
            toml::from_str::<GamepadBindingsConfig>(&printed).unwrap(),
            defaults
        );
    }

    #[test]
    fn playstation_and_xbox_names_select_the_same_buttons() {
        let config: GamepadBindingsConfig = toml::from_str(
            "[typed_keys]\ncross = \"Return\"\nb = \"Escape\"\ncreate = \"Tab\"\nmenu = \
             \"space\"\n[compositor_actions]\nl1 = \"move_left\"\nps = \"toggle_overview\"\n",
        )
        .unwrap();
        assert_eq!(
            config.role_of(GamepadButton::Back),
            Some(ButtonRole::TypedKey("Tab"))
        );
        assert_eq!(
            config.role_of(GamepadButton::Start),
            Some(ButtonRole::TypedKey("space"))
        );
        assert_eq!(
            config.role_of(GamepadButton::South),
            Some(ButtonRole::TypedKey("Return"))
        );
        assert_eq!(
            config.role_of(GamepadButton::East),
            Some(ButtonRole::TypedKey("Escape"))
        );
        assert_eq!(
            config.role_of(GamepadButton::LeftShoulder),
            Some(ButtonRole::Compositor(CompositorAction::MoveLeft))
        );
        assert_eq!(
            config.role_of(GamepadButton::Guide),
            Some(ButtonRole::Compositor(CompositorAction::ToggleOverview))
        );
    }

    #[test]
    fn each_section_claims_buttons_for_its_purpose() {
        let config: GamepadBindingsConfig = toml::from_str(
            "[compositor_actions]\nwest = \"close_window\"\n[commands]\nstart = [\"notify-send\", \
             \"hi\"]\n",
        )
        .unwrap();
        assert_eq!(
            config.role_of(GamepadButton::West),
            Some(ButtonRole::Compositor(CompositorAction::CloseWindow))
        );
        assert_eq!(
            config.role_of(GamepadButton::Start),
            Some(ButtonRole::Command(&[
                "notify-send".to_owned(),
                "hi".to_owned()
            ]))
        );
        assert_eq!(
            config.role_of(GamepadButton::DpadLeft),
            Some(ButtonRole::TypedKey("Left"))
        );
    }

    #[test]
    fn the_defaults_claim_every_button_at_most_once() {
        let config = GamepadBindingsConfig::default();
        assert!(config.conflicting_buttons().is_empty());
        assert_eq!(config.stick_sharing(), StickSharing::Separate);
    }

    #[test]
    fn a_button_in_two_sections_is_reported_and_the_first_purpose_wins() {
        let mut config = GamepadBindingsConfig::default();
        config
            .compositor_actions
            .insert(GamepadButton::LeftTrigger, CompositorAction::CloseWindow);
        assert_eq!(
            config.conflicting_buttons(),
            BTreeSet::from([GamepadButton::LeftTrigger])
        );
        assert_eq!(
            config.role_of(GamepadButton::LeftTrigger),
            Some(ButtonRole::PushToTalk)
        );
    }

    #[test]
    fn the_right_trigger_holds_the_left_mouse_button() {
        let steering = GamepadBindingsConfig::default().pointer_steering();
        assert_eq!(
            steering.buttons.get(&GamepadButton::RightTrigger),
            Some(&PointerButton::Left)
        );
        assert_eq!(
            steering.stick.map(|stick| stick.stick),
            Some(GamepadStick::Right)
        );
    }

    #[test]
    fn the_right_stick_press_holds_the_right_mouse_button() {
        let steering = GamepadBindingsConfig::default().pointer_steering();
        assert_eq!(
            steering.buttons.get(&GamepadButton::RightStick),
            Some(&PointerButton::Right)
        );
    }

    #[test]
    fn games_matching_a_passive_prefix_silence_the_bindings() {
        let config = GamepadBindingsConfig::default();
        assert_eq!(
            config.focus_behaviour(Some("steam_app_1091500")),
            FocusBehaviour::Passive
        );
        assert_eq!(
            config.focus_behaviour(Some("steam")),
            FocusBehaviour::Passive
        );
        assert_eq!(
            config.focus_behaviour(Some("kitty")),
            FocusBehaviour::Active
        );
        assert_eq!(config.focus_behaviour(None), FocusBehaviour::Active);
    }
}
