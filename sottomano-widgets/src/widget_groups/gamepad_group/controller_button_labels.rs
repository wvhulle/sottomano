use sottomano_config::widget_groups::gamepad_bindings::GamepadButton;

const SONY_USB_VENDOR: u16 = 0x054c;
const NINTENDO_USB_VENDOR: u16 = 0x057e;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerFamily {
    PlayStation,
    Nintendo,
    Xbox,
}

impl ControllerFamily {
    #[must_use]
    pub const fn of_vendor(vendor_id: u16) -> Self {
        match vendor_id {
            SONY_USB_VENDOR => ControllerFamily::PlayStation,
            NINTENDO_USB_VENDOR => ControllerFamily::Nintendo,
            _ => ControllerFamily::Xbox,
        }
    }

    #[must_use]
    pub fn described_button(self, button: GamepadButton) -> String {
        let label = self.button_label(button);
        let position = button.position_name();
        if label == position {
            label.to_owned()
        } else {
            format!("{label} · {position}")
        }
    }

    #[must_use]
    pub const fn button_layout_name(self) -> &'static str {
        match self {
            ControllerFamily::PlayStation => "PlayStation",
            ControllerFamily::Nintendo => "Nintendo",
            ControllerFamily::Xbox => "Xbox",
        }
    }

    #[must_use]
    pub const fn button_label(self, button: GamepadButton) -> &'static str {
        match (self, button) {
            (
                _,
                GamepadButton::DpadUp
                | GamepadButton::DpadDown
                | GamepadButton::DpadLeft
                | GamepadButton::DpadRight,
            ) => button.position_name(),
            (ControllerFamily::PlayStation, button) => playstation_label(button),
            (ControllerFamily::Nintendo, button) => nintendo_label(button),
            (ControllerFamily::Xbox, button) => xbox_label(button),
        }
    }
}

const fn playstation_label(button: GamepadButton) -> &'static str {
    match button {
        GamepadButton::South => "Cross",
        GamepadButton::East => "Circle",
        GamepadButton::North => "Triangle",
        GamepadButton::West => "Square",
        GamepadButton::LeftShoulder => "L1",
        GamepadButton::RightShoulder => "R1",
        GamepadButton::LeftTrigger => "L2",
        GamepadButton::RightTrigger => "R2",
        GamepadButton::Back => "Create",
        GamepadButton::Start => "Options",
        GamepadButton::Guide => "PS",
        GamepadButton::LeftStick => "L3",
        GamepadButton::RightStick => "R3",
        GamepadButton::DpadUp
        | GamepadButton::DpadDown
        | GamepadButton::DpadLeft
        | GamepadButton::DpadRight => "D-pad",
    }
}

const fn nintendo_label(button: GamepadButton) -> &'static str {
    match button {
        GamepadButton::South => "B",
        GamepadButton::East => "A",
        GamepadButton::North => "X",
        GamepadButton::West => "Y",
        GamepadButton::LeftShoulder => "L",
        GamepadButton::RightShoulder => "R",
        GamepadButton::LeftTrigger => "ZL",
        GamepadButton::RightTrigger => "ZR",
        GamepadButton::Back => "Minus",
        GamepadButton::Start => "Plus",
        GamepadButton::Guide => "Home",
        GamepadButton::LeftStick => "Left stick",
        GamepadButton::RightStick => "Right stick",
        GamepadButton::DpadUp
        | GamepadButton::DpadDown
        | GamepadButton::DpadLeft
        | GamepadButton::DpadRight => "D-pad",
    }
}

const fn xbox_label(button: GamepadButton) -> &'static str {
    match button {
        GamepadButton::South => "A",
        GamepadButton::East => "B",
        GamepadButton::North => "Y",
        GamepadButton::West => "X",
        GamepadButton::LeftShoulder => "LB",
        GamepadButton::RightShoulder => "RB",
        GamepadButton::LeftTrigger => "LT",
        GamepadButton::RightTrigger => "RT",
        GamepadButton::Back => "View",
        GamepadButton::Start => "Menu",
        GamepadButton::Guide => "Xbox",
        GamepadButton::LeftStick => "LS",
        GamepadButton::RightStick => "RS",
        GamepadButton::DpadUp
        | GamepadButton::DpadDown
        | GamepadButton::DpadLeft
        | GamepadButton::DpadRight => "D-pad",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_south_face_button_is_named_after_each_family() {
        let south = |vendor| ControllerFamily::of_vendor(vendor).button_label(GamepadButton::South);
        assert_eq!(south(SONY_USB_VENDOR), "Cross");
        assert_eq!(south(NINTENDO_USB_VENDOR), "B");
        assert_eq!(south(0x045e), "A");
    }

    #[test]
    fn buttons_are_described_by_label_and_position() {
        let playstation = ControllerFamily::PlayStation;
        assert_eq!(
            playstation.described_button(GamepadButton::RightTrigger),
            "R2 · right trigger"
        );
        assert_eq!(
            playstation.described_button(GamepadButton::DpadUp),
            "D-pad up"
        );
    }
}
