use iced::widget::{
    Text, text,
    text::{IntoFragment, Shaping},
};

use crate::user_config::font;

/// A text widget pre-styled with the bar's resolved font, size, and advanced
/// shaping (needed for workspace names / window titles with special chars).
/// Callers can still chain `.color(…)` etc. on the returned builder.
pub fn bar_text<'a>(content: impl IntoFragment<'a>) -> Text<'a, iced::Theme, iced::Renderer> {
    text(content)
        .size(font::size())
        .font(font::iced_font())
        .shaping(Shaping::Advanced)
}
