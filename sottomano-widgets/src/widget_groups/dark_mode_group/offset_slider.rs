use chrono::TimeDelta;

use super::{DarkModeTarget, OFFSET_MINUTES, OFFSET_STEP_MINUTES};
use crate::{
    widget_groups::dark_mode_group::auto_switch_toggle::format_offset_minutes,
    widgets::{slider_control::SliderControl, weather_icons::WeatherIcon},
};

pub(super) fn new_offset_slider(
    name: &'static str,
    target: DarkModeTarget,
    icon: WeatherIcon,
) -> SliderControl {
    SliderControl::new(name, OFFSET_MINUTES, target, icon)
        .scroll_step(OFFSET_STEP_MINUTES)
        .slider_step(OFFSET_STEP_MINUTES)
        .readout(|minutes| format_offset_minutes(minutes_to_time_delta(minutes)))
}

pub(super) fn minutes_to_time_delta(minutes: f32) -> TimeDelta {
    TimeDelta::minutes(minutes.round() as i64)
}

pub(super) fn time_delta_to_minutes(offset: TimeDelta) -> f32 {
    offset.num_minutes() as f32
}
