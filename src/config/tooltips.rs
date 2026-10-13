use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::layout::spacing::{Length, Spacing};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    default,
    deny_unknown_fields,
    bound(deserialize = "L: Deserialize<'de> + Length")
)]
pub struct TooltipsConfig<L = f32> {
    pub value_width: L,
    #[serde(with = "humantime_serde")]
    pub popup_morph: Duration,
    #[serde(with = "humantime_serde")]
    pub copy_flash: Duration,
    pub loading: TooltipLoading,
}

impl<L: Length> Default for TooltipsConfig<L> {
    fn default() -> Self {
        Self {
            value_width: L::px(620.0),
            popup_morph: Duration::from_millis(160),
            copy_flash: Duration::from_millis(700),
            loading: TooltipLoading::default(),
        }
    }
}

impl TooltipsConfig<Spacing> {
    pub fn resolve(self, em_px: f32) -> TooltipsConfig {
        TooltipsConfig {
            value_width: self.value_width.resolve(em_px),
            popup_morph: self.popup_morph,
            copy_flash: self.copy_flash,
            loading: self.loading,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TooltipLoading {
    #[serde(with = "humantime_serde")]
    pub debug_delay: Duration,
    pub placeholder: PlaceholderEffect,
    pub intro: IntroEffect,
    pub replace: ReplaceEffect,
}

impl Default for TooltipLoading {
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
