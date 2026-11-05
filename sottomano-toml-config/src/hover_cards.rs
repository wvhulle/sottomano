use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HoverCardsConfig {
    #[serde(with = "humantime_serde")]
    pub copy_flash_duration: Duration,
    pub loading: HoverCardLoading,
}

impl Default for HoverCardsConfig {
    fn default() -> Self {
        Self {
            copy_flash_duration: Duration::from_millis(700),
            loading: HoverCardLoading::default(),
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
