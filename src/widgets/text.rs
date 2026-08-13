use std::collections::HashMap;

use iced::{
    Color, Font, Pixels, Size,
    advanced::{
        graphics::text::Paragraph,
        text::{self as advanced_text, Paragraph as _},
    },
    alignment::Vertical,
    font::{Style, Weight},
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

#[derive(Clone, Copy)]
pub struct TextStyle {
    font: Font,
    size: f32,
}

impl TextStyle {
    pub fn of(font: &FontConfig) -> Self {
        Self {
            font: font.iced_font,
            size: font.text_size_px,
        }
    }

    pub fn bold(self) -> Self {
        Self {
            font: Font {
                weight: Weight::Bold,
                ..self.font
            },
            ..self
        }
    }

    pub fn italic(self) -> Self {
        Self {
            font: Font {
                style: Style::Italic,
                ..self.font
            },
            ..self
        }
    }

    pub fn scaled(self, factor: f32) -> Self {
        Self {
            size: self.size * factor,
            ..self
        }
    }

    pub fn line_height(self) -> f32 {
        LINE_HEIGHT.to_absolute(Pixels(self.size)).0
    }

    pub fn measure(self, content: &str) -> Size {
        self.measure_wrapped(content, f32::INFINITY)
    }

    pub fn measure_wrapped(self, content: &str, width: f32) -> Size {
        if content.is_empty() {
            return Size::ZERO;
        }
        Paragraph::with_text(self.layout(content, width)).min_bounds()
    }

    fn layout(self, content: &str, width: f32) -> advanced_text::Text<&str> {
        advanced_text::Text {
            content,
            bounds: Size::new(width, f32::INFINITY),
            size: Pixels(self.size),
            line_height: LINE_HEIGHT,
            font: self.font,
            align_x: advanced_text::Alignment::Left,
            align_y: Vertical::Top,
            shaping: advanced_text::Shaping::Advanced,
            wrapping: advanced_text::Wrapping::Word,
        }
    }
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
