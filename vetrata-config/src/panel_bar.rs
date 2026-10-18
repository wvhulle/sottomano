use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::layout::spacing::Spacing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct PanelBarConfig<L = f32> {
    pub slider_width: L,
    pub indicator_thickness: L,
    pub frame_outline: L,
    pub drag_start_distance: L,
    pub drag_ghost_scale: f32,
    #[serde(with = "humantime_serde")]
    pub drag_spring_load: Duration,
    #[serde(with = "humantime_serde")]
    pub group_expansion: Duration,
    #[serde(with = "humantime_serde")]
    pub slider_expansion: Duration,
    #[serde(with = "humantime_serde")]
    pub focus_indicator_motion: Duration,
}

impl<L: From<f32>> Default for PanelBarConfig<L> {
    fn default() -> Self {
        Self {
            slider_width: L::from(60.0),
            indicator_thickness: L::from(2.4),
            frame_outline: L::from(1.0),
            drag_start_distance: L::from(6.0),
            drag_ghost_scale: 1.08,
            drag_spring_load: Duration::from_millis(600),
            group_expansion: Duration::from_millis(90),
            slider_expansion: Duration::from_millis(90),
            focus_indicator_motion: Duration::from_millis(160),
        }
    }
}

impl PanelBarConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> PanelBarConfig {
        PanelBarConfig {
            slider_width: self.slider_width.resolve(em_px),
            indicator_thickness: self.indicator_thickness.resolve(em_px),
            frame_outline: self.frame_outline.resolve(em_px),
            drag_start_distance: self.drag_start_distance.resolve(em_px),
            drag_ghost_scale: self.drag_ghost_scale,
            drag_spring_load: self.drag_spring_load,
            group_expansion: self.group_expansion,
            slider_expansion: self.slider_expansion,
            focus_indicator_motion: self.focus_indicator_motion,
        }
    }
}
