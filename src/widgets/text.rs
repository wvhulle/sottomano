use std::collections::HashMap;

use iced::{
    Color, Font,
    widget::{
        Text, span, text,
        text::{IntoFragment, Shaping, Span},
    },
};

use crate::config::resolved::ResolvedFontConfig;

/// A text widget pre-styled with the bar font, size, and advanced shaping
/// (needed for workspace names and window titles with special characters).
pub fn bar_text<'a>(
    font: &ResolvedFontConfig,
    content: impl IntoFragment<'a>,
) -> Text<'a, iced::Theme, iced::Renderer> {
    text(content)
        .size(font.text_size_px)
        .font(font.iced_font)
        .shaping(Shaping::Advanced)
}

/// Split `content` into rich-text spans: the bar font for glyphs it has, the
/// pre-resolved `fallbacks` for glyphs it lacks (so glyphs the bar font is
/// missing still match the rest of the desktop), and `base` for uncovered
/// glyphs with no fallback yet. `color` styles every span; `None` keeps the
/// theme default.
pub fn fallback_spans<'a>(
    font: &ResolvedFontConfig,
    content: &str,
    base: Font,
    color: Option<Color>,
    fallbacks: &HashMap<char, Font>,
) -> Vec<Span<'a>> {
    let tagged: Vec<(char, Font)> = content
        .chars()
        .map(|ch| {
            let glyph_font = if font.covered.contains(&ch) {
                base
            } else {
                fallbacks.get(&ch).copied().unwrap_or(base)
            };
            (ch, glyph_font)
        })
        .collect();
    tagged
        .chunk_by(|a, b| a.1 == b.1)
        .map(|run| {
            let text: String = run.iter().map(|&(ch, _)| ch).collect();
            span(text).font(run[0].1).color_maybe(color)
        })
        .collect()
}
