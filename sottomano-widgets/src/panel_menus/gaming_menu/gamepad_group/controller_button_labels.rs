use iced_controller_diagram::ControllerModel;
use sottomano_desktop_sources::gamepad_controller_input::steam_dongle_hidraw::VALVE_USB_VENDOR;
use sottomano_toml_config::panel_menus::gaming_menu::gamepad_bindings::GamepadButton;

use crate::glyph_icons::{
    icon_glyph::IconGlyph,
    phosphor::{Phosphor, Weight},
    simple_icons::SimpleIcon,
};

const SONY_USB_VENDOR: u16 = 0x054c;
const NINTENDO_USB_VENDOR: u16 = 0x057e;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerFamily {
    PlayStation,
    Nintendo,
    Xbox,
    Steam,
}

impl ControllerFamily {
    #[must_use]
    pub const fn controller_model(self) -> ControllerModel {
        match self {
            ControllerFamily::PlayStation => ControllerModel::DualSense,
            ControllerFamily::Nintendo => ControllerModel::SwitchPro,
            ControllerFamily::Xbox => ControllerModel::XboxSeries,
            ControllerFamily::Steam => ControllerModel::SteamController,
        }
    }

    #[must_use]
    pub fn brand_glyph(self) -> IconGlyph {
        match self {
            ControllerFamily::PlayStation => SimpleIcon::PlayStation.into(),
            ControllerFamily::Nintendo | ControllerFamily::Xbox | ControllerFamily::Steam => {
                Phosphor::GameController(Weight::Regular).into()
            }
        }
    }

    #[must_use]
    pub const fn button_layout_name(self) -> &'static str {
        match self {
            ControllerFamily::PlayStation => "PlayStation",
            ControllerFamily::Nintendo => "Nintendo",
            ControllerFamily::Xbox => "Xbox",
            ControllerFamily::Steam => "Steam",
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
            (ControllerFamily::Steam, button) => steam_label(button),
        }
    }
}

impl From<u16> for ControllerFamily {
    fn from(vendor_id: u16) -> Self {
        match vendor_id {
            SONY_USB_VENDOR => ControllerFamily::PlayStation,
            NINTENDO_USB_VENDOR => ControllerFamily::Nintendo,
            VALVE_USB_VENDOR => ControllerFamily::Steam,
            _ => ControllerFamily::Xbox,
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

const fn steam_label(button: GamepadButton) -> &'static str {
    match button {
        GamepadButton::Guide => "Steam",
        GamepadButton::LeftStick => "L3",
        GamepadButton::RightStick => "R3",
        other => xbox_label(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_south_face_button_is_named_after_each_family() {
        let south = |vendor| ControllerFamily::from(vendor).button_label(GamepadButton::South);
        assert_eq!(south(SONY_USB_VENDOR), "Cross");
        assert_eq!(south(NINTENDO_USB_VENDOR), "B");
        assert_eq!(south(0x045e), "A");
        assert_eq!(south(VALVE_USB_VENDOR), "A");
    }

    #[test]
    fn steam_controllers_name_their_own_guide_button() {
        let steam = ControllerFamily::from(VALVE_USB_VENDOR);
        assert_eq!(steam.button_label(GamepadButton::Guide), "Steam");
        assert_eq!(steam.controller_model(), ControllerModel::SteamController);
    }
}
