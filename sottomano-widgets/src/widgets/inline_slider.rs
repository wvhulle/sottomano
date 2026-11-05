use std::ops::RangeInclusive;

use corredo::{
    inline_slider::{SliderAccents, inline_slider as themed_inline_slider},
    theme_colors::{primary_color, text_color},
};
use iced::{Color, Element, Theme, mouse::ScrollDelta};
use sottomano_toml_config::{Config, severity::Severity};

use crate::group_items::reading_display_state::ReadingDisplayState;

fn accent_for_level(level: Severity, normal: fn(&Theme) -> Color) -> Box<dyn Fn(&Theme) -> Color> {
    Box::new(move |theme| match level {
        Severity::Normal => normal(theme),
        other => ReadingDisplayState::from(other).color(theme),
    })
}

#[allow(clippy::too_many_arguments)]
pub fn inline_slider<'a, Message: Clone + 'a>(
    config: &Config,
    range: RangeInclusive<f32>,
    value: f32,
    lagging_value: Option<f32>,
    step: f32,
    level: Severity,
    on_change: impl Fn(f32) -> Message + 'a,
    on_release: Message,
    on_scroll: impl Fn(ScrollDelta) -> Message + 'a,
) -> Element<'a, Message> {
    themed_inline_slider(range, value, on_change)
        .icon_size(config.theme.font.icon_size_px)
        .lagging_behind(lagging_value)
        .step(step)
        .accents(SliderAccents {
            resting: accent_for_level(level, text_color),
            engaged: accent_for_level(level, primary_color),
        })
        .on_release(on_release)
        .on_scroll(on_scroll)
        .into()
}
