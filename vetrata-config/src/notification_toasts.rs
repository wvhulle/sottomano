use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{
    layout::spacing::Spacing,
    screen_edges::{HEdge, VEdge},
    severity::Severity,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct NotificationToastsConfig<L = f32> {
    pub placement: ToastPlacementConfig<L>,
    pub lifetime: ToastLifetimeConfig,
    pub card: ToastCardConfig<L>,
}

impl<L: From<f32>> Default for NotificationToastsConfig<L> {
    fn default() -> Self {
        Self {
            placement: ToastPlacementConfig::default(),
            lifetime: ToastLifetimeConfig::default(),
            card: ToastCardConfig::default(),
        }
    }
}

impl NotificationToastsConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> NotificationToastsConfig {
        NotificationToastsConfig {
            placement: self.placement.resolve(em_px),
            lifetime: self.lifetime,
            card: self.card.resolve(em_px),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct ToastCardConfig<L = f32> {
    pub width: L,
    pub icon_size: L,
    pub app_badge_size: L,
    pub image_preview_max_height: L,
    #[serde(with = "humantime_serde")]
    pub appear: Duration,
    #[serde(with = "humantime_serde")]
    pub dismiss_fade: Duration,
    #[serde(with = "humantime_serde")]
    pub expire_fade: Duration,
    #[serde(with = "humantime_serde")]
    pub motion: Duration,
}

impl<L: From<f32>> Default for ToastCardConfig<L> {
    fn default() -> Self {
        Self {
            width: L::from(440.0),
            icon_size: L::from(44.0),
            app_badge_size: L::from(20.0),
            image_preview_max_height: L::from(180.0),
            appear: Duration::from_millis(90),
            dismiss_fade: Duration::from_millis(140),
            expire_fade: Duration::from_millis(220),
            motion: Duration::from_millis(160),
        }
    }
}

impl ToastCardConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> ToastCardConfig {
        ToastCardConfig {
            width: self.width.resolve(em_px),
            icon_size: self.icon_size.resolve(em_px),
            app_badge_size: self.app_badge_size.resolve(em_px),
            image_preview_max_height: self.image_preview_max_height.resolve(em_px),
            appear: self.appear,
            dismiss_fade: self.dismiss_fade,
            expire_fade: self.expire_fade,
            motion: self.motion,
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
    #[must_use]
    pub const fn vertical_edge(self) -> VEdge {
        match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => VEdge::Top,
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => VEdge::Bottom,
        }
    }

    #[must_use]
    pub const fn horizontal_edge(self) -> HEdge {
        match self {
            Self::TopLeft | Self::BottomLeft => HEdge::Left,
            Self::TopCenter | Self::BottomCenter => HEdge::Center,
            Self::TopRight | Self::BottomRight => HEdge::Right,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct ToastPlacementConfig<L = f32> {
    pub fallback_corner: FallbackCorner,
    pub edge_gap: L,
    pub window_right_offset: L,
    pub window_top_offset: L,
}

impl<L: From<f32>> Default for ToastPlacementConfig<L> {
    fn default() -> Self {
        Self {
            fallback_corner: FallbackCorner::BottomRight,
            edge_gap: L::from(24.0),
            window_right_offset: L::from(12.0),
            window_top_offset: L::from(48.0),
        }
    }
}

impl ToastPlacementConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> ToastPlacementConfig {
        ToastPlacementConfig {
            fallback_corner: self.fallback_corner,
            edge_gap: self.edge_gap.resolve(em_px),
            window_right_offset: self.window_right_offset.resolve(em_px),
            window_top_offset: self.window_top_offset.resolve(em_px),
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
    #[must_use]
    pub fn default_timeout_for(&self, severity: Severity) -> Option<Duration> {
        let configured = match severity {
            Severity::Low => self.low_severity_timeout,
            Severity::Normal | Severity::Warning => self.normal_severity_timeout,
            Severity::Critical => self.critical_severity_timeout,
        };
        (!configured.is_zero()).then_some(configured)
    }
}
