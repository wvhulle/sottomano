use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::data::{
    severity::Severity,
    source::niri::{HEdge, VEdge},
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct NotificationToastsConfig {
    pub placement: ToastPlacementConfig,
    pub lifetime: ToastLifetimeConfig,
    pub card: ToastCardConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ToastCardConfig {
    pub width_px: f32,
    pub icon_size_px: f32,
    pub app_badge_size_px: f32,
    pub image_preview_max_height_px: f32,
    #[serde(with = "humantime_serde")]
    pub appear: Duration,
    #[serde(with = "humantime_serde")]
    pub dismiss_fade: Duration,
    #[serde(with = "humantime_serde")]
    pub expire_fade: Duration,
    #[serde(with = "humantime_serde")]
    pub motion: Duration,
}

impl Default for ToastCardConfig {
    fn default() -> Self {
        Self {
            width_px: 440.0,
            icon_size_px: 44.0,
            app_badge_size_px: 20.0,
            image_preview_max_height_px: 180.0,
            appear: Duration::from_millis(90),
            dismiss_fade: Duration::from_millis(140),
            expire_fade: Duration::from_millis(220),
            motion: Duration::from_millis(160),
        }
    }
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct ToastLifetimeConfig {
    #[serde(with = "humantime_serde")]
    pub low_severity_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub normal_severity_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub critical_severity_timeout: Duration,
}

impl Default for ToastLifetimeConfig {
    fn default() -> Self {
        Self {
            low_severity_timeout: Duration::from_secs(8),
            normal_severity_timeout: Duration::from_secs(12),
            critical_severity_timeout: Duration::ZERO,
        }
    }
}

impl ToastLifetimeConfig {
    pub fn default_timeout_for(&self, severity: Severity) -> Option<Duration> {
        let configured = match severity {
            Severity::Low => self.low_severity_timeout,
            Severity::Normal | Severity::Warning => self.normal_severity_timeout,
            Severity::Critical => self.critical_severity_timeout,
        };
        (!configured.is_zero()).then_some(configured)
    }
}
