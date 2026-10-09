use serde::{Deserialize, Serialize};

use crate::data::source::niri::{HEdge, VEdge};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FallbackCorner {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl FallbackCorner {
    pub const fn vertical_edge(self) -> VEdge {
        match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => VEdge::Top,
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => VEdge::Bottom,
        }
    }

    pub const fn horizontal_edge(self) -> HEdge {
        match self {
            Self::TopLeft | Self::BottomLeft => HEdge::Left,
            Self::TopCenter | Self::BottomCenter => HEdge::Center,
            Self::TopRight | Self::BottomRight => HEdge::Right,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToastPlacementConfig {
    pub fallback_corner: FallbackCorner,
    pub edge_gap_px: f32,
    pub window_right_offset_px: f32,
    pub window_top_offset_px: f32,
}

impl Default for ToastPlacementConfig {
    fn default() -> Self {
        Self {
            fallback_corner: FallbackCorner::BottomRight,
            edge_gap_px: 24.0,
            window_right_offset_px: 12.0,
            window_top_offset_px: 48.0,
        }
    }
}
