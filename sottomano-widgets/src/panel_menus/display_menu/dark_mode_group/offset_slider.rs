use chrono::TimeDelta;

use super::{OFFSET_MINUTES, OFFSET_STEP_MINUTES};
use crate::{
    glyph_icons::weather_icons::WeatherIcon, group_indicators::slider_control::SliderControl,
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
