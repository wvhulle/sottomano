use iced_controller_diagram::{DiagramAnchor, DiagramButton, DiagramStick};
use sottomano_toml_config::widget_groups::gamepad_bindings::{GamepadButton, GamepadStick};

pub(super) const fn button_anchor(button: GamepadButton) -> DiagramAnchor {
    DiagramAnchor::Button(match button {
        GamepadButton::South => DiagramButton::South,
        GamepadButton::East => DiagramButton::East,
        GamepadButton::North => DiagramButton::North,
        GamepadButton::West => DiagramButton::West,
        GamepadButton::DpadUp => DiagramButton::DpadUp,
        GamepadButton::DpadDown => DiagramButton::DpadDown,
        GamepadButton::DpadLeft => DiagramButton::DpadLeft,
        GamepadButton::DpadRight => DiagramButton::DpadRight,
        GamepadButton::LeftShoulder => DiagramButton::LeftShoulder,
        GamepadButton::RightShoulder => DiagramButton::RightShoulder,
        GamepadButton::LeftTrigger => DiagramButton::LeftTrigger,
        GamepadButton::RightTrigger => DiagramButton::RightTrigger,
        GamepadButton::Back => DiagramButton::Back,
        GamepadButton::Start => DiagramButton::Start,
        GamepadButton::Guide => DiagramButton::Guide,
        GamepadButton::LeftStick => DiagramButton::LeftStick,
        GamepadButton::RightStick => DiagramButton::RightStick,
    })
}

pub(super) const fn stick_anchor(stick: GamepadStick) -> DiagramAnchor {
    DiagramAnchor::Stick(match stick {
        GamepadStick::Left => DiagramStick::Left,
        GamepadStick::Right => DiagramStick::Right,
    })
}
