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
    pub const fn of_vendor(vendor_id: Option<u16>) -> Self {
        match vendor_id {
            Some(SONY_USB_VENDOR) => ControllerFamily::PlayStation,
            Some(NINTENDO_USB_VENDOR) => ControllerFamily::Nintendo,
            _ => ControllerFamily::Xbox,
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
            (_, GamepadButton::DpadUp) => "D-pad up",
            (_, GamepadButton::DpadDown) => "D-pad down",
            (_, GamepadButton::DpadLeft) => "D-pad left",
            (_, GamepadButton::DpadRight) => "D-pad right",
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
        GamepadButton::LeftBumper => "L1",
        GamepadButton::RightBumper => "R1",
        GamepadButton::LeftTrigger => "L2",
        GamepadButton::RightTrigger => "R2",
        GamepadButton::Select => "Create",
        GamepadButton::Start => "Options",
        GamepadButton::Mode => "PS",
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
        GamepadButton::LeftBumper => "L",
        GamepadButton::RightBumper => "R",
        GamepadButton::LeftTrigger => "ZL",
        GamepadButton::RightTrigger => "ZR",
        GamepadButton::Select => "Minus",
        GamepadButton::Start => "Plus",
        GamepadButton::Mode => "Home",
        GamepadButton::LeftStick => "Left stick press",
        GamepadButton::RightStick => "Right stick press",
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
        GamepadButton::LeftBumper => "LB",
        GamepadButton::RightBumper => "RB",
        GamepadButton::LeftTrigger => "LT",
        GamepadButton::RightTrigger => "RT",
        GamepadButton::Select => "View",
        GamepadButton::Start => "Menu",
        GamepadButton::Mode => "Guide",
        GamepadButton::LeftStick => "LS press",
        GamepadButton::RightStick => "RS press",
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
        assert_eq!(south(Some(SONY_USB_VENDOR)), "Cross");
        assert_eq!(south(Some(NINTENDO_USB_VENDOR)), "B");
        assert_eq!(south(None), "A");
    }
}
