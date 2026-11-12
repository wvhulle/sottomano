use iced::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextEffects {
    pub placeholder: PlaceholderEffect,
    pub intro: IntroEffect,
    pub replace: ReplaceEffect,
}

impl Default for TextEffects {
    fn default() -> Self {
        Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderEffect {
    Pulse { period: Duration },
    Shimmer { sweep_period: Duration },
    Noise { glyph_change: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroEffect {
    Fade {
        duration: Duration,
    },
    Decode {
        duration: Duration,
        glyph_change: Duration,
    },
    Wipe {
        duration: Duration,
    },
    Typewriter {
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
                per_char
                    * u32::try_from(text.chars().count())
                        .expect("an animated text has fewer than u32::MAX characters")
            }
            IntroEffect::Immediate => Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceEffect {
    Decode {
        duration: Duration,
        glyph_change: Duration,
    },
    Crossfade {
        duration: Duration,
    },
    Roll {
        duration: Duration,
    },
    Flash {
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
