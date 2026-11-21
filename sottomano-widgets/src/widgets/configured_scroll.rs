use corredo::{
    orientation::Axis,
    selection_scroll::{ScrollPacing, SelectionFollowingScroll, selection_following_scroll},
};
use iced::Element;
use sottomano_toml_config::Config;

pub fn configured_scroll<'view, Message: 'view>(
    content: impl Into<Element<'view, Message>>,
    axis: Axis,
    config: &Config,
) -> SelectionFollowingScroll<'view, Message> {
    selection_following_scroll(content, axis)
        .wheel_line_travel(config.scrolling.wheel_line_px)
        .paced(ScrollPacing::Eased(config.scrolling.wheel_easing))
}
