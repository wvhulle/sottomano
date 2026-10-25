use chrono::{NaiveTime, TimeDelta, Timelike};

use super::{CLOCK_MINUTES, CLOCK_STEP_MINUTES, QuietScheduleTarget};
use crate::widgets::{slider_control::SliderControl, weather_icons::WeatherIcon};

pub(super) fn new_clock_slider(
    name: &'static str,
    target: QuietScheduleTarget,
    icon: WeatherIcon,
) -> SliderControl {
    SliderControl::new(name, CLOCK_MINUTES, target, icon)
        .scroll_step(CLOCK_STEP_MINUTES)
        .slider_step(CLOCK_STEP_MINUTES)
        .readout(|minutes, _| clock_text(clock_at(minutes)))
        .label(|minutes, _| Some(clock_text(clock_at(minutes))))
}

#[allow(clippy::cast_precision_loss)]
pub(super) fn clock_minutes(time: NaiveTime) -> f32 {
    (time.hour() * 60 + time.minute()) as f32
}

#[allow(clippy::cast_possible_truncation)]
pub(super) fn clock_at(minutes: f32) -> NaiveTime {
    NaiveTime::MIN + TimeDelta::minutes(minutes.round().max(0.0) as i64)
}

pub(super) fn clock_text(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
}
