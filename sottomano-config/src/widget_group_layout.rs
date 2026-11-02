use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::spacing::Spacing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct WidgetGroupLayoutConfig<L = f32> {
    pub edge_indicator_thickness: L,
    #[serde(with = "humantime_serde")]
    pub expansion_duration: Duration,
}

impl<L: From<f32>> Default for WidgetGroupLayoutConfig<L> {
    fn default() -> Self {
        Self {
            edge_indicator_thickness: L::from(2.4),
            expansion_duration: Duration::from_millis(90),
        }
    }
}

impl WidgetGroupLayoutConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> WidgetGroupLayoutConfig {
        WidgetGroupLayoutConfig {
            edge_indicator_thickness: self.edge_indicator_thickness.resolve(em_px),
            expansion_duration: self.expansion_duration,
        }
    }
}
