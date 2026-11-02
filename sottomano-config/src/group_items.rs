use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::spacing::Spacing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct GroupItemsConfig<L = f32> {
    pub item_gap: L,
    pub slider_width: L,
    pub slider_stroke_width: L,
    #[serde(with = "humantime_serde")]
    pub slider_expansion_duration: Duration,
}

impl<L: From<f32>> Default for GroupItemsConfig<L> {
    fn default() -> Self {
        Self {
            item_gap: L::from(4.0),
            slider_width: L::from(60.0),
            slider_stroke_width: L::from(2.0),
            slider_expansion_duration: Duration::from_millis(90),
        }
    }
}

impl GroupItemsConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> GroupItemsConfig {
        GroupItemsConfig {
            item_gap: self.item_gap.resolve(em_px),
            slider_width: self.slider_width.resolve(em_px),
            slider_stroke_width: self.slider_stroke_width.resolve(em_px),
            slider_expansion_duration: self.slider_expansion_duration,
        }
    }
}
