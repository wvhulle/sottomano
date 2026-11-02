use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::spacing::Spacing;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + From<f32>")
)]
pub struct HoverCardsConfig<L = f32> {
    pub value_column_width: L,
    #[serde(with = "humantime_serde")]
    pub copy_flash_duration: Duration,
    pub loading: HoverCardLoading,
}

impl<L: From<f32>> Default for HoverCardsConfig<L> {
    fn default() -> Self {
        Self {
            value_column_width: L::from(620.0),
            copy_flash_duration: Duration::from_millis(700),
            loading: HoverCardLoading::default(),
        }
    }
}

impl HoverCardsConfig<Spacing> {
    #[must_use]
    pub fn resolve(self, em_px: f32) -> HoverCardsConfig {
        HoverCardsConfig {
            value_column_width: self.value_column_width.resolve(em_px),
            copy_flash_duration: self.copy_flash_duration,
            loading: self.loading,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HoverCardLoading {
    #[serde(with = "humantime_serde")]
    pub debug_delay: Duration,
    pub placeholder: PlaceholderEffect,
    pub intro: IntroEffect,
    pub replace: ReplaceEffect,
}

impl Default for HoverCardLoading {
    fn default() -> Self {
        Self {
            debug_delay: Duration::ZERO,
            placeholder: PlaceholderEffect::Pulse {
                period: Duration::from_millis(1400),
            },
            intro: IntroEffect::Fade {
                duration: Duration::from_millis(500),
            },
            replace: ReplaceEffect::Decode {
                duration: Duration::from_millis(350),
                glyph_change: Duration::from_millis(55),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlaceholderEffect {
    Pulse {
        #[serde(with = "humantime_serde")]
        period: Duration,
    },
    Shimmer {
        #[serde(with = "humantime_serde")]
        sweep_period: Duration,
    },
    Noise {
        #[serde(with = "humantime_serde")]
        glyph_change: Duration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntroEffect {
    Fade {
        #[serde(with = "humantime_serde")]
        duration: Duration,
    },
    Decode {
        #[serde(with = "humantime_serde")]
        duration: Duration,
        #[serde(with = "humantime_serde")]
        glyph_change: Duration,
    },
    Wipe {
        #[serde(with = "humantime_serde")]
        duration: Duration,
    },
    Typewriter {
        #[serde(with = "humantime_serde")]
        per_char: Duration,
    },
    Immediate,
}

impl IntroEffect {
    #[must_use]
    pub fn length(self, text: &str) -> Duration {
        match self {
            IntroEffect::Fade { duration }
            | IntroEffect::Decode { duration, .. }
            | IntroEffect::Wipe { duration } => duration,
            IntroEffect::Typewriter { per_char } => {
                per_char.saturating_mul(u32::try_from(text.chars().count()).unwrap_or(u32::MAX))
            }
            IntroEffect::Immediate => Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplaceEffect {
    Decode {
        #[serde(with = "humantime_serde")]
        duration: Duration,
        #[serde(with = "humantime_serde")]
        glyph_change: Duration,
    },
    Crossfade {
        #[serde(with = "humantime_serde")]
        duration: Duration,
    },
    Roll {
        #[serde(with = "humantime_serde")]
        duration: Duration,
    },
    Flash {
        #[serde(with = "humantime_serde")]
        duration: Duration,
    },
    Immediate,
}

impl ReplaceEffect {
    #[must_use]
    pub const fn length(self) -> Duration {
        match self {
            ReplaceEffect::Decode { duration, .. }
            | ReplaceEffect::Crossfade { duration }
            | ReplaceEffect::Roll { duration }
            | ReplaceEffect::Flash { duration } => duration,
            ReplaceEffect::Immediate => Duration::ZERO,
        }
    }
}
