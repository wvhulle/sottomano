use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::spacing::Spacing;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct DesktopPanelsConfig<L = f32> {
    pub height: L,
    pub slot_drag: PanelSlotDragConfig,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanelSlotDragConfig {
    pub ghost_scale: f32,
    #[serde(with = "humantime_serde")]
    pub spring_load_delay: Duration,
    #[serde(with = "humantime_serde")]
    pub focus_indicator_duration: Duration,
}

impl<L: From<f32>> Default for DesktopPanelsConfig<L> {
    fn default() -> Self {
        Self {
            height: L::from(24.0),
            slot_drag: PanelSlotDragConfig::default(),
        }
    }
}

impl Default for PanelSlotDragConfig {
    fn default() -> Self {
        Self {
            ghost_scale: 1.08,
            spring_load_delay: Duration::from_millis(600),
            focus_indicator_duration: Duration::from_millis(160),
        }
    }
}

impl DesktopPanelsConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> DesktopPanelsConfig {
        DesktopPanelsConfig {
            height: self.height.resolve(em_px),
            slot_drag: self.slot_drag,
        }
    }
}
