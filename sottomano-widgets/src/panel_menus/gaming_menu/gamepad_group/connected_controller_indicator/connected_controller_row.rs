use sottomano_desktop_sources::gamepad_controller_input::evdev_gamepad_devices::{
    ConnectedGamepad, PhysicalGamepadLink,
};
use sottomano_toml_config::panel_menus::gaming_menu::gamepad_bindings::GamepadBindingsConfig;

use super::ConnectedControllerIndicator;
use crate::{
    group_indicators::group_indicator::GroupIndicator,
    panel_menus::gaming_menu::gamepad_group::GamepadGroup,
};

impl GamepadGroup {
    pub(in crate::panel_menus::gaming_menu::gamepad_group) fn connected_controller_row<'view>(
        &'view self,
        gamepad: &'view ConnectedGamepad,
        link: &'view PhysicalGamepadLink,
        bindings: &'view GamepadBindingsConfig,
    ) -> GroupIndicator<'view> {
        ConnectedControllerIndicator {
            gamepad,
            bindings,
            custody: self.controller_custody(gamepad.slot),
            marking: self.controller_marking(gamepad, link),
        }
        .indicator()
    }
}
