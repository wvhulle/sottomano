use serde::Deserialize;

use crate::data::source::niri::{HEdge, VEdge};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

pub struct Edges {
    pub vertical: VEdge,
    pub horizontal: HEdge,
}

impl Corner {
    pub fn edges(self) -> Edges {
        let (vertical, horizontal) = match self {
            Self::TopLeft => (VEdge::Top, HEdge::Left),
            Self::TopCenter => (VEdge::Top, HEdge::Center),
            Self::TopRight => (VEdge::Top, HEdge::Right),
            Self::BottomLeft => (VEdge::Bottom, HEdge::Left),
            Self::BottomCenter => (VEdge::Bottom, HEdge::Center),
            Self::BottomRight => (VEdge::Bottom, HEdge::Right),
        };
        Edges {
            vertical,
            horizontal,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NotificationConfig {
    pub fallback_position: Corner,
    pub edge_gap: f32,
    pub window_right_offset: f32,
    pub window_top_offset: f32,
    pub card_width: f32,
    pub app_icon_size: f32,
    pub image_size: f32,
    pub badge_size: f32,
    pub low_timeout_ms: u64,
    pub normal_timeout_ms: u64,
    pub critical_timeout_ms: u64,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            fallback_position: Corner::BottomRight,
            edge_gap: 24.0,
            window_right_offset: 12.0,
            window_top_offset: 48.0,
            card_width: 380.0,
            app_icon_size: 40.0,
            image_size: 48.0,
            badge_size: 20.0,
            low_timeout_ms: 8_000,
            normal_timeout_ms: 12_000,
            critical_timeout_ms: 0,
        }
    }
}
