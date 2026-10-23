use sottomano_config::widget_groups::gamepad_bindings::GamepadBindingsConfig;
use sottomano_sources::gamepad_controller_input::{
    evdev_gamepad_devices::{ConnectedGamepad, GamepadPower},
    gamepad_input_machine::GamepadInput,
};
use sottomano_widgets::{
    clock::Clock,
    widget_groups::gamepad_group::{GamepadGroup, GamepadMessage},
};

use crate::showcased_group::{ShowcasedGamepad, ShowcasedGroup};

pub const DUALSENSE_SLOT: usize = 31;

pub fn group(_clock: Clock) -> ShowcasedGroup {
    let bindings = GamepadBindingsConfig::default();
    let mut group = GamepadGroup::default();
    let _ = group.update(
        GamepadMessage::Input(GamepadInput::Roster(vec![ConnectedGamepad {
            slot: DUALSENSE_SLOT,
            name: "DualSense Wireless Controller".to_owned(),
            vendor_id: 0x054c,
            power: GamepadPower::Discharging(72),
        }])),
        &bindings,
        None,
    );
    ShowcasedGroup::Gamepad(Box::new(ShowcasedGamepad { group, bindings }))
}
