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
