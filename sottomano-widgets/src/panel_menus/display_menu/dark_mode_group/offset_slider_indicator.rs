use chrono::TimeDelta;
use iced::Function;
use sottomano_toml_config::Config;

use super::{
    ColorMode, DarkModeGroup, DarkModeInteraction, DarkModeSource, OFFSET_MINUTES,
    OFFSET_STEP_MINUTES,
};
use crate::{
    glyph_icons::weather_icons::WeatherIcon,
    group_indicators::{group_indicator::GroupIndicator, slider_control::SliderControl},
    widgets::localized_durations::signed_duration_text,
};

pub(super) fn new_offset_slider(name: &'static str, icon: WeatherIcon) -> SliderControl {
    SliderControl::new(name, OFFSET_MINUTES, icon)
        .scroll_step(OFFSET_STEP_MINUTES)
        .slider_step(OFFSET_STEP_MINUTES)
        .readout(|minutes, locale| signed_duration_text(minutes_to_time_delta(minutes), locale))
}

pub(super) fn minutes_to_time_delta(minutes: f32) -> TimeDelta {
    TimeDelta::minutes(minutes.round() as i64)
}

pub(super) fn time_delta_to_minutes(offset: TimeDelta) -> f32 {
    offset.num_minutes() as f32
}

impl DarkModeGroup {
    pub(super) fn offset_slider_row<'view>(
        &'view self,
        config: &'view Config,
        edge: ColorMode,
    ) -> GroupIndicator<'view> {
        GroupIndicator::slider(
            self.offset_slider_for(edge),
            config,
            DarkModeInteraction::OffsetSlider.with(edge),
        )
        .refreshed_by(DarkModeInteraction::Refresh(DarkModeSource::Location))
    }
}
