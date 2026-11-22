use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HoverCardsConfig {
    #[serde(with = "crate::friendly_duration")]
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
    #[serde(with = "crate::friendly_duration")]
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
        #[serde(with = "crate::friendly_duration")]
        period: Duration,
    },
    Shimmer {
        #[serde(with = "crate::friendly_duration")]
        sweep_period: Duration,
    },
    Noise {
        #[serde(with = "crate::friendly_duration")]
        glyph_change: Duration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntroEffect {
    Fade {
        #[serde(with = "crate::friendly_duration")]
        duration: Duration,
    },
    Decode {
        #[serde(with = "crate::friendly_duration")]
        duration: Duration,
        #[serde(with = "crate::friendly_duration")]
        glyph_change: Duration,
    },
    Wipe {
        #[serde(with = "crate::friendly_duration")]
        duration: Duration,
    },
    Typewriter {
        #[serde(with = "crate::friendly_duration")]
        per_char: Duration,
    },
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "effect", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReplaceEffect {
    Decode {
        #[serde(with = "crate::friendly_duration")]
        duration: Duration,
        #[serde(with = "crate::friendly_duration")]
        glyph_change: Duration,
    },
    Crossfade {
        #[serde(with = "crate::friendly_duration")]
        duration: Duration,
    },
    Roll {
        #[serde(with = "crate::friendly_duration")]
        duration: Duration,
    },
    Flash {
        #[serde(with = "crate::friendly_duration")]
        duration: Duration,
    },
    Immediate,
}
