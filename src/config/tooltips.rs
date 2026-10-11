use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TooltipsConfig {
    pub value_width_px: f32,
    pub caption_width_px: f32,
    #[serde(with = "humantime_serde")]
    pub popup_morph: Duration,
    #[serde(with = "humantime_serde")]
    pub copy_flash: Duration,
    pub loading: TooltipLoading,
}

impl Default for TooltipsConfig {
    fn default() -> Self {
        Self {
            value_width_px: 620.0,
            caption_width_px: 300.0,
            popup_morph: Duration::from_millis(160),
            copy_flash: Duration::from_millis(700),
            loading: TooltipLoading::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TooltipLoading {
    pub effect: TooltipLoadingEffect,
    #[serde(with = "humantime_serde")]
    pub debug_delay: Duration,
    pub decode: DecodeTiming,
    pub skeleton: SkeletonTiming,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TooltipLoadingEffect {
    #[default]
    Decode,
    Skeleton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct DecodeTiming {
    #[serde(with = "humantime_serde")]
    pub settle: Duration,
    #[serde(with = "humantime_serde")]
    pub glyph_change: Duration,
}

impl Default for DecodeTiming {
    fn default() -> Self {
        Self {
            settle: Duration::from_millis(700),
            glyph_change: Duration::from_millis(55),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkeletonTiming {
    #[serde(with = "humantime_serde")]
    pub pulse_period: Duration,
}

impl Default for SkeletonTiming {
    fn default() -> Self {
        Self {
            pulse_period: Duration::from_millis(1400),
        }
    }
}
