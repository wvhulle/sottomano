use std::collections::HashMap;

use iced::{
    Color, Font, Task,
    advanced::{graphics::text::font_system, text as advanced_text},
    font::{Family, Style, Weight},
    task::Handle,
    widget::{
        Text, span, text,
        text::{IntoFragment, Shaping, Span},
    },
};
use vetrata_config::theme::{
    Theme,
    font::{FontRole, ResolvedFont, fallback_family},
};
use vetrata_sources::background_work::blocking;

pub const LINE_HEIGHT: advanced_text::LineHeight = advanced_text::LineHeight::Relative(1.1);

pub fn column_heading<'a>(
    font: &ResolvedFont,
    title: impl IntoFragment<'a>,
    color: Color,
) -> Text<'a, iced::Theme, iced::Renderer> {
    bar_text(font, title)
        .font(FontFaces::from(font).italic)
        .color(color)
}

pub fn bar_text<'a>(
    font: &ResolvedFont,
    content: impl IntoFragment<'a>,
) -> Text<'a, iced::Theme, iced::Renderer> {
    text(content)
        .size(font.text_size_px)
        .font(FontFaces::from(font).regular)
        .line_height(LINE_HEIGHT)
        .shaping(Shaping::Advanced)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontFaces {
    pub regular: Font,
    pub bold: Font,
    pub italic: Font,
}

impl From<&ResolvedFont> for FontFaces {
    fn from(font: &ResolvedFont) -> Self {
        let regular = Font {
            family: match font.role {
                FontRole::Text => Family::SansSerif,
                FontRole::Monospace => Family::Monospace,
            },
            ..Font::DEFAULT
        };
        Self {
            regular,
            bold: Font {
                weight: Weight::Bold,
                ..regular
            },
            italic: Font {
                style: Style::Italic,
                ..regular
            },
        }
    }
}

#[derive(Default)]
pub struct FontFallbacks {
    fonts: HashMap<char, Font>,
    resolve: Option<Handle>,
}

impl FontFallbacks {
    #[must_use]
    pub fn naming_theme_families(theme: &Theme) -> Self {
        let mut fonts = font_system().write().expect("font system lock poisoned");
        let database = fonts.raw().db_mut();
        database.set_sans_serif_family(theme.font.family.clone());
        database.set_monospace_family(theme.monospace_font.family.clone());
        Self::default()
    }

    fn font_for(&self, ch: char, base: Font) -> Font {
        self.fonts.get(&ch).copied().unwrap_or(base)
    }

    pub fn merge(&mut self, resolved: HashMap<char, Font>) {
        self.fonts.extend(resolved);
    }

    pub fn resolve_missing(
        &mut self,
        content: impl Iterator<Item = char>,
        font: &ResolvedFont,
    ) -> Task<HashMap<char, Font>> {
        let pending: Vec<char> = content
            .filter(|ch| !font.covered.contains(ch) && !self.fonts.contains_key(ch))
            .collect();
        if pending.is_empty() {
            return Task::none();
        }
        let base = FontFaces::from(font).regular;
        let (task, handle) = Task::future(blocking("fallback fonts", move || {
            pending
                .into_iter()
                .map(|ch| {
                    let fallback = fallback_family(ch)
                        .map_or(base, |family| Font::with_name(String::leak(family)));
                    (ch, fallback)
                })
                .collect::<HashMap<char, Font>>()
        }))
        .abortable();
        self.resolve = Some(handle.abort_on_drop());
        task
    }
}

impl FontFallbacks {
    #[must_use]
    pub fn runs(&self, font: &ResolvedFont, content: &str, base: Font) -> Vec<(String, Font)> {
        let tagged: Vec<(char, Font)> = content
            .chars()
            .map(|ch| {
                let glyph_font = if font.covered.contains(&ch) {
                    base
                } else {
                    self.font_for(ch, base)
                };
                (ch, glyph_font)
            })
            .collect();
        tagged
            .chunk_by(|a, b| a.1 == b.1)
            .map(|run| (run.iter().map(|&(ch, _)| ch).collect(), run[0].1))
            .collect()
    }
}

#[must_use]
pub fn colored_span<'a>(text: String, font: Font, color: Color) -> Span<'a> {
    span(text).font(font).color(color)
}
