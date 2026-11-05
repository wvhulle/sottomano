use corredo::toggle_switch::toggle_switch;
use iced::Element;
use sottomano_toml_config::Config;

#[must_use]
pub fn item_toggle_switch<'a, Message: 'a>(
    config: &Config,
    switched_on: bool,
    opacity: f32,
) -> Element<'a, Message> {
    toggle_switch(config.theme.font.icon_size_px, switched_on, opacity)
}
