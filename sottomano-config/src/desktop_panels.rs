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
    pub frame_outline_width: L,
    pub group_edge_padding: L,
    pub joined_group_gap: L,
    pub framed_group_gap: L,
    pub slot_drag: PanelSlotDragConfig<L>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct PanelSlotDragConfig<L = f32> {
    pub start_distance: L,
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
            frame_outline_width: L::from(1.0),
            group_edge_padding: L::from(5.0),
            joined_group_gap: L::from(11.0),
            framed_group_gap: L::from(4.0),
            slot_drag: PanelSlotDragConfig::default(),
        }
    }
}

impl<L: From<f32>> Default for PanelSlotDragConfig<L> {
    fn default() -> Self {
        Self {
            start_distance: L::from(6.0),
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
            frame_outline_width: self.frame_outline_width.resolve(em_px),
            group_edge_padding: self.group_edge_padding.resolve(em_px),
            joined_group_gap: self.joined_group_gap.resolve(em_px),
            framed_group_gap: self.framed_group_gap.resolve(em_px),
            slot_drag: self.slot_drag.resolve(em_px),
        }
    }
}

impl PanelSlotDragConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> PanelSlotDragConfig {
        PanelSlotDragConfig {
            start_distance: self.start_distance.resolve(em_px),
            ghost_scale: self.ghost_scale,
            spring_load_delay: self.spring_load_delay,
            focus_indicator_duration: self.focus_indicator_duration,
        }
    }
}
