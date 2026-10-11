use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanelBarConfig {
    pub height_px: f32,
    pub popup_gap_px: f32,
    pub group_padding_px: f32,
    pub widget_spacing_px: f32,
    pub group_spacing_px: f32,
    pub slider_width_px: f32,
    pub indicator_thickness_px: f32,
    pub frame_outline_px: f32,
    #[serde(with = "humantime_serde")]
    pub group_expansion: Duration,
    #[serde(with = "humantime_serde")]
    pub slider_expansion: Duration,
    #[serde(with = "humantime_serde")]
    pub focus_indicator_motion: Duration,
}

impl Default for PanelBarConfig {
    fn default() -> Self {
        Self {
            height_px: 24.0,
            popup_gap_px: 14.0,
            group_padding_px: 5.0,
            widget_spacing_px: 4.0,
            group_spacing_px: 11.0,
            slider_width_px: 60.0,
            indicator_thickness_px: 2.4,
            frame_outline_px: 1.0,
            group_expansion: Duration::from_millis(90),
            slider_expansion: Duration::from_millis(90),
            focus_indicator_motion: Duration::from_millis(160),
        }
    }
}
