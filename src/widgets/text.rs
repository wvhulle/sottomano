use iced::{
    Color, Font,
    widget::{
        Text, span, text,
        text::{IntoFragment, Shaping, Span},
    },
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

/// Rich-text spans for `content`, each run rendered with the font
/// [`font::font_for_char`] selects — the bar font where it covers the glyph, a
/// fontconfig-picked font where it doesn't. This makes glyphs the bar font lacks
/// (e.g. the `⠐` progress marker in terminal titles) match the rest of the
/// desktop instead of cosmic-text's hardcoded per-script fallback. `color` styles
/// every span (`None` keeps the theme's default text color); adjacent characters
/// sharing a font are merged into one span. Wrap the result in
/// [`iced::widget::rich_text`].
pub fn fallback_spans<'a>(content: &str, base: Font, color: Option<Color>) -> Vec<Span<'a>> {
    let tagged: Vec<(char, Font)> = content
        .chars()
        .map(|ch| (ch, font::font_for_char(ch, base)))
        .collect();
    tagged
        .chunk_by(|a, b| a.1 == b.1)
        .map(|run| {
            let text: String = run.iter().map(|&(ch, _)| ch).collect();
            span(text).font(run[0].1).color_maybe(color)
        })
        .collect()
}
