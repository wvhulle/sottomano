use corredo::{
    orientation::Axis,
    selection_scroll::{ScrollPacing, SelectionFollowingScroll, selection_following_scroll},
};
use iced::Element;
use sottomano_toml_config::Config;

pub fn configured_scroll<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    axis: Axis,
    config: &Config,
) -> SelectionFollowingScroll<'a, Message> {
    selection_following_scroll(content, axis)
        .wheel_line_travel(config.scrolling.wheel_line_px)
        .paced(ScrollPacing::Eased(config.scrolling.wheel_easing))
}
