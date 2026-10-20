use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamepadButton {
    South,
    East,
    North,
    West,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftBumper,
    RightBumper,
    LeftTrigger,
    RightTrigger,
    Select,
    Start,
    Mode,
    LeftStick,
    RightStick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositorAction {
    FocusColumnLeft,
    FocusColumnRight,
    FocusWindowUp,
    FocusWindowDown,
    FocusWorkspaceUp,
    FocusWorkspaceDown,
    FocusWindowPrevious,
    MoveColumnLeft,
    MoveColumnRight,
    MoveWindowToWorkspaceUp,
    MoveWindowToWorkspaceDown,
    ToggleOverview,
    MaximizeColumn,
    CenterColumn,
    CloseWindow,
}

impl CompositorAction {
    #[must_use]
    pub const fn press_hint(self) -> &'static str {
        match self {
            CompositorAction::FocusColumnLeft => "focus the column to the left",
            CompositorAction::FocusColumnRight => "focus the column to the right",
            CompositorAction::FocusWindowUp => "focus the window above",
            CompositorAction::FocusWindowDown => "focus the window below",
            CompositorAction::FocusWorkspaceUp => "focus the workspace above",
            CompositorAction::FocusWorkspaceDown => "focus the workspace below",
            CompositorAction::FocusWindowPrevious => "focus the previous window",
            CompositorAction::MoveColumnLeft => "move the column left",
            CompositorAction::MoveColumnRight => "move the column right",
            CompositorAction::MoveWindowToWorkspaceUp => "move the window a workspace up",
            CompositorAction::MoveWindowToWorkspaceDown => "move the window a workspace down",
            CompositorAction::ToggleOverview => "toggle the overview",
            CompositorAction::MaximizeColumn => "maximize the column",
            CompositorAction::CenterColumn => "center the column",
            CompositorAction::CloseWindow => "close the window",
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
    pub const fn press_hint(self) -> &'static str {
        match self {
            PointerButton::Left => "hold the left mouse button",
            PointerButton::Right => "hold the right mouse button",
            PointerButton::Middle => "hold the middle mouse button",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonBinding {
    Compositor(CompositorAction),
    Command(Vec<String>),
    PointerButton(PointerButton),
}

impl ButtonBinding {
    #[must_use]
    pub fn press_hint(&self) -> String {
        match self {
            ButtonBinding::Compositor(action) => action.press_hint().to_owned(),
            ButtonBinding::Command(command) => format!("run {}", command.join(" ")),
            ButtonBinding::PointerButton(button) => button.press_hint().to_owned(),
        }
    }

    fn command(program_and_args: &[&str]) -> Self {
        ButtonBinding::Command(
            program_and_args
                .iter()
                .map(|&part| part.to_owned())
                .collect(),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PressReleaseBindings {
    pub press: Option<ButtonBinding>,
    pub release: Option<ButtonBinding>,
}

impl PressReleaseBindings {
    fn on_press(binding: ButtonBinding) -> Self {
        Self {
            press: Some(binding),
            release: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GamepadStick {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StickDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StickRole {
    #[default]
    Ignore,
    Flicks,
    Pointer,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct StickBindings {
    pub role: StickRole,
    pub up: Option<ButtonBinding>,
    pub down: Option<ButtonBinding>,
    pub left: Option<ButtonBinding>,
    pub right: Option<ButtonBinding>,
    pub pointer_pixels_per_second: u32,
}

impl Default for StickBindings {
    fn default() -> Self {
        Self {
            role: StickRole::Ignore,
            up: None,
            down: None,
            left: None,
            right: None,
            pointer_pixels_per_second: 1600,
        }
    }
}

impl StickBindings {
    #[must_use]
    pub fn flick_binding(&self, direction: StickDirection) -> Option<&ButtonBinding> {
        if self.role != StickRole::Flicks {
            return None;
        }
        match direction {
            StickDirection::Up => self.up.as_ref(),
            StickDirection::Down => self.down.as_ref(),
            StickDirection::Left => self.left.as_ref(),
            StickDirection::Right => self.right.as_ref(),
        }
    }

    fn flicking(
        up: CompositorAction,
        down: CompositorAction,
        left: CompositorAction,
        right: CompositorAction,
    ) -> Self {
        Self {
            role: StickRole::Flicks,
            up: Some(ButtonBinding::Compositor(up)),
            down: Some(ButtonBinding::Compositor(down)),
            left: Some(ButtonBinding::Compositor(left)),
            right: Some(ButtonBinding::Compositor(right)),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GamepadSticksConfig {
    pub left: StickBindings,
    pub right: StickBindings,
}

impl GamepadSticksConfig {
    #[must_use]
    pub fn of(&self, stick: GamepadStick) -> &StickBindings {
        match stick {
            GamepadStick::Left => &self.left,
            GamepadStick::Right => &self.right,
        }
    }

    #[must_use]
    pub fn pointer_stick(&self) -> Option<PointerStickSettings> {
        [GamepadStick::Left, GamepadStick::Right]
            .into_iter()
            .find(|&stick| self.of(stick).role == StickRole::Pointer)
            .map(|stick| PointerStickSettings {
                stick,
                pixels_per_second: self.of(stick).pointer_pixels_per_second,
            })
    }
}

impl Default for GamepadSticksConfig {
    fn default() -> Self {
        Self {
            left: StickBindings::flicking(
                CompositorAction::FocusWorkspaceUp,
                CompositorAction::FocusWorkspaceDown,
                CompositorAction::FocusColumnLeft,
                CompositorAction::FocusColumnRight,
            ),
            right: StickBindings {
                role: StickRole::Pointer,
                ..StickBindings::default()
            },
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
    pub fn needs_virtual_pointer(&self) -> bool {
        self.stick.is_some() || !self.buttons.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GamepadBindingsConfig {
    pub passive_app_id_prefixes: Vec<String>,
    pub handover_chord: Vec<GamepadButton>,
    pub buttons: BTreeMap<GamepadButton, PressReleaseBindings>,
    pub sticks: GamepadSticksConfig,
}

impl GamepadBindingsConfig {
    #[must_use]
    pub fn pointer_steering(&self) -> PointerSteeringSettings {
        PointerSteeringSettings {
            stick: self.sticks.pointer_stick(),
            buttons: self
                .buttons
                .iter()
                .filter_map(|(&button, bound)| match bound.press {
                    Some(ButtonBinding::PointerButton(pointer_button)) => {
                        Some((button, pointer_button))
                    }
                    _ => None,
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn completes_handover_chord(
        &self,
        pressed: GamepadButton,
        held: impl Fn(GamepadButton) -> bool,
    ) -> bool {
        self.handover_chord.contains(&pressed)
            && self.handover_chord.iter().all(|&button| held(button))
    }

    #[must_use]
    pub fn acts_while_focused(&self, app_id: Option<&str>) -> bool {
        app_id.is_none_or(|app_id| {
            !self
                .passive_app_id_prefixes
                .iter()
                .any(|prefix| app_id.starts_with(prefix.as_str()))
        })
    }
}

impl Default for GamepadBindingsConfig {
    fn default() -> Self {
        let compositor = |action| PressReleaseBindings::on_press(ButtonBinding::Compositor(action));
        let typed_key =
            |key| PressReleaseBindings::on_press(ButtonBinding::command(&["wtype", "-k", key]));
        Self {
            passive_app_id_prefixes: ["steam", "gamescope"].map(String::from).to_vec(),
            handover_chord: vec![GamepadButton::Select, GamepadButton::Start],
            sticks: GamepadSticksConfig::default(),
            buttons: BTreeMap::from([
                (GamepadButton::DpadLeft, typed_key("Left")),
                (GamepadButton::DpadRight, typed_key("Right")),
                (GamepadButton::DpadUp, typed_key("Up")),
                (GamepadButton::DpadDown, typed_key("Down")),
                (GamepadButton::South, typed_key("Return")),
                (GamepadButton::East, typed_key("Escape")),
                (
                    GamepadButton::RightTrigger,
                    PressReleaseBindings::on_press(ButtonBinding::PointerButton(
                        PointerButton::Left,
                    )),
                ),
                (
                    GamepadButton::LeftBumper,
                    compositor(CompositorAction::MoveColumnLeft),
                ),
                (
                    GamepadButton::RightBumper,
                    compositor(CompositorAction::MoveColumnRight),
                ),
                (
                    GamepadButton::Mode,
                    compositor(CompositorAction::ToggleOverview),
                ),
                (
                    GamepadButton::North,
                    compositor(CompositorAction::MaximizeColumn),
                ),
                (
                    GamepadButton::LeftTrigger,
                    PressReleaseBindings {
                        press: Some(ButtonBinding::command(&["voxtype", "record", "start"])),
                        release: Some(ButtonBinding::command(&["voxtype", "record", "stop"])),
                    },
                ),
            ]),
        }
    }
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
    fn buttons_bind_compositor_actions_and_commands() {
        let config: GamepadBindingsConfig = toml::from_str(
            "[buttons.west]\npress = { compositor = \"close_window\" \
             }\n[buttons.right_stick]\npress = { command = [\"notify-send\", \"hi\"] }\n",
        )
        .unwrap();
        assert_eq!(
            config.buttons[&GamepadButton::West].press,
            Some(ButtonBinding::Compositor(CompositorAction::CloseWindow))
        );
        assert_eq!(
            config.buttons[&GamepadButton::RightStick].press,
            Some(ButtonBinding::command(&["notify-send", "hi"]))
        );
    }

    #[test]
    fn games_matching_a_passive_prefix_silence_the_bindings() {
        let config = GamepadBindingsConfig::default();
        assert!(!config.acts_while_focused(Some("steam_app_1091500")));
        assert!(!config.acts_while_focused(Some("steam")));
        assert!(config.acts_while_focused(Some("kitty")));
        assert!(config.acts_while_focused(None));
    }

    #[test]
    fn the_handover_chord_completes_only_when_every_button_is_held() {
        let config = GamepadBindingsConfig::default();
        let start_and_select_held =
            |button| matches!(button, GamepadButton::Select | GamepadButton::Start);
        assert!(config.completes_handover_chord(GamepadButton::Start, start_and_select_held));
        assert!(
            !config.completes_handover_chord(GamepadButton::Start, |button| {
                button == GamepadButton::Start
            })
        );
        assert!(!config.completes_handover_chord(GamepadButton::South, start_and_select_held));
    }

    #[test]
    fn the_right_trigger_holds_the_left_mouse_button() {
        let steering = GamepadBindingsConfig::default().pointer_steering();
        assert_eq!(
            steering.buttons.get(&GamepadButton::RightTrigger),
            Some(&PointerButton::Left)
        );
        assert!(steering.needs_virtual_pointer());
    }

    #[test]
    fn the_left_stick_flicks_between_windows_and_the_right_stick_steers_the_pointer() {
        let sticks = GamepadSticksConfig::default();
        assert_eq!(
            sticks.left.flick_binding(StickDirection::Right),
            Some(&ButtonBinding::Compositor(
                CompositorAction::FocusColumnRight
            ))
        );
        assert_eq!(sticks.right.flick_binding(StickDirection::Right), None);
        assert_eq!(
            sticks.pointer_stick().map(|pointer| pointer.stick),
            Some(GamepadStick::Right)
        );
    }
}
