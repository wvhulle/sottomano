mod animated_text;
pub use animated_text::{
    AnimatedText, TextPresence, TransitionStyle, TransitioningText, animated_text,
};
mod steady_width_readout;
pub use steady_width_readout::{ReadoutLabels, SteadyWidthReadout, steady_width_readout};
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
