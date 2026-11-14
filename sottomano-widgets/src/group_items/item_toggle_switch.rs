use corredo::value_controls::{TogglePosition, toggle_switch};
use iced::Element;
use sottomano_toml_config::Config;

use crate::group_items::toggle_control::SwitchPosition;

#[must_use]
pub fn item_toggle_switch<'a, Message: 'a>(
    config: &Config,
    switch_position: SwitchPosition,
    opacity: f32,
) -> Element<'a, Message> {
    toggle_switch(match switch_position {
        SwitchPosition::On => TogglePosition::On,
        SwitchPosition::Off => TogglePosition::Off,
    })
    .size(config.theme.font.icon_size_px)
    .opacity(opacity)
    .into()
}
