use std::collections::HashMap;

use iced::{
    Color, Font,
    advanced::text as advanced_text,
    widget::{
        Text, span, text,
        text::{IntoFragment, Shaping, Span},
    },
};

use crate::config::font::FontConfig;

pub const LINE_HEIGHT: advanced_text::LineHeight = advanced_text::LineHeight::Relative(1.1);

pub fn bar_text<'a>(
    font: &FontConfig,
    content: impl IntoFragment<'a>,
) -> Text<'a, iced::Theme, iced::Renderer> {
    text(content)
        .size(font.text_size_px)
        .font(font.iced_font)
        .line_height(LINE_HEIGHT)
        .shaping(Shaping::Advanced)
}

pub fn fallback_spans<'a>(
    font: &FontConfig,
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
