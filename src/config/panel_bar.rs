use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::layout::spacing::{Length, Spacing};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + Length")
)]
pub struct PanelBarConfig<L = f32> {
    pub slider_width: L,
    pub indicator_thickness: L,
    pub frame_outline: L,
    #[serde(with = "humantime_serde")]
    pub group_expansion: Duration,
    #[serde(with = "humantime_serde")]
    pub slider_expansion: Duration,
    #[serde(with = "humantime_serde")]
    pub focus_indicator_motion: Duration,
}

impl<L: Length> Default for PanelBarConfig<L> {
    fn default() -> Self {
        Self {
            slider_width: L::px(60.0),
            indicator_thickness: L::px(2.4),
            frame_outline: L::px(1.0),
            group_expansion: Duration::from_millis(90),
            slider_expansion: Duration::from_millis(90),
            focus_indicator_motion: Duration::from_millis(160),
        }
    }
}

impl PanelBarConfig<Spacing> {
    pub fn resolve(self, em_px: f32) -> PanelBarConfig {
        PanelBarConfig {
            slider_width: self.slider_width.resolve(em_px),
            indicator_thickness: self.indicator_thickness.resolve(em_px),
            frame_outline: self.frame_outline.resolve(em_px),
            group_expansion: self.group_expansion,
            slider_expansion: self.slider_expansion,
            focus_indicator_motion: self.focus_indicator_motion,
        }
    }
}
