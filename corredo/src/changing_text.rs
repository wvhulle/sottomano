mod animated_text;
pub use animated_text::{
    AnimatedText, TextPresence, TransitionStyle, TransitioningText, animated_text,
};
mod steady_width_text;
pub use steady_width_text::{
    ContentGrowth, LabelShape, MeasuredReadout, ReadoutLabels, SteadyWidthText, WidthSource,
    steady_width_text,
};
#[cfg(feature = "karaoke")]
mod karaoke_line;
#[cfg(feature = "karaoke")]
pub use karaoke_line::{KaraokeColors, KaraokeLine, LineSweep, SweepMotion};
#[cfg(feature = "markup")]
mod markup_text;
#[cfg(feature = "markup")]
pub use markup_text::MarkupText;
mod text_effects;
pub use text_effects::{IntroEffect, PlaceholderEffect, ReplaceEffect, TextEffects};
