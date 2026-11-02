use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{
    screen_edges::{HEdge, VEdge},
    severity::Severity,
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
#[serde(default, deny_unknown_fields)]
pub struct ToastPlacementConfig {
    pub fallback_corner: FallbackCorner,
}

impl Default for ToastPlacementConfig {
    fn default() -> Self {
        Self {
            fallback_corner: FallbackCorner::BottomRight,
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
