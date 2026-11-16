use corredo::{
    theming::{primary_color, text_color},
    value_controls::{SliderAccents, SliderLook},
};
use iced::{Color, Theme};
use sottomano_toml_config::{Config, severity::Severity};

use crate::group_indicators::indicator_state::IndicatorState;

fn accent_for_level(level: Severity, normal: fn(&Theme) -> Color) -> Box<dyn Fn(&Theme) -> Color> {
    Box::new(move |theme| match level {
        Severity::Normal => normal(theme),
        other => IndicatorState::from(other).color(theme),
    })
}

#[must_use]
pub fn slider_look(config: &Config, level: Severity) -> SliderLook<'static> {
    SliderLook {
        icon_size_px: config.theme.font.icon_size_px,
        accents: SliderAccents {
            resting: accent_for_level(level, text_color),
            engaged: accent_for_level(level, primary_color),
        },
    }
}
