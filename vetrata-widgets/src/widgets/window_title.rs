use iced::{
    Element, Font,
    widget::{
        rich_text,
        text::{Span, Wrapping},
    },
};
use vetrata_config::Config;
use vetrata_sources::window_label::WindowLabel;

use crate::widgets::{
    palette_colors::PaletteColors,
    text::{FontFaces, FontFallbacks, LINE_HEIGHT, colored_span},
};

const SEPARATOR: &str = " · ";

pub struct WindowTitle<'a> {
    label: &'a WindowLabel,
    config: &'a Config,
    fallbacks: &'a FontFallbacks,
    size: f32,
    opacity: f32,
    wrapping: Wrapping,
}

impl<'a> WindowTitle<'a> {
    #[must_use]
    pub fn new(label: &'a WindowLabel, config: &'a Config, fallbacks: &'a FontFallbacks) -> Self {
        Self {
            label,
            config,
            fallbacks,
            size: config.theme.font.text_size_px,
            opacity: 1.0,
            wrapping: Wrapping::default(),
        }
    }

    #[must_use]
    pub fn size(self, size: f32) -> Self {
        Self { size, ..self }
    }

    #[must_use]
    pub fn opacity(self, opacity: f32) -> Self {
        Self { opacity, ..self }
    }

    #[must_use]
    pub fn wrapping(self, wrapping: Wrapping) -> Self {
        Self { wrapping, ..self }
    }

    fn main_spans(&self, content: &str) -> Vec<Span<'static>> {
        self.styled(content, FontFaces::from(&self.config.theme.font).regular)
    }

    fn context_spans(&self, content: &str) -> Vec<Span<'static>> {
        self.styled(content, FontFaces::from(&self.config.theme.font).italic)
    }

    fn separator_spans(&self) -> Vec<Span<'static>> {
        self.styled(SEPARATOR, FontFaces::from(&self.config.theme.font).regular)
    }

    fn styled(&self, content: &str, base: Font) -> Vec<Span<'static>> {
        let color = PaletteColors::from(&self.config.theme.palette)
            .bar_foreground
            .scale_alpha(self.opacity);
        self.fallbacks
            .runs(&self.config.theme.font, content, base)
            .into_iter()
            .map(|(text, font)| colored_span(text, font, color))
            .collect()
    }

    fn spans(&self) -> Vec<Span<'static>> {
        match self.label {
            WindowLabel::KittyAgent(kitty) => self
                .main_spans(kitty.working_location.display_text())
                .into_iter()
                .chain(kitty.foreground_activity.iter().flat_map(|activity| {
                    self.separator_spans()
                        .into_iter()
                        .chain(self.context_spans(activity.display_text()))
                }))
                .collect(),
            WindowLabel::Firefox { page, .. } | WindowLabel::BrandedBrowser(page) => page
                .site
                .iter()
                .flat_map(|site| {
                    self.context_spans(site)
                        .into_iter()
                        .chain(self.separator_spans())
                })
                .chain(self.main_spans(&page.page))
                .collect(),
            WindowLabel::Titled(title) => self.main_spans(title),
        }
    }
}

impl<'a, Message: 'a> From<WindowTitle<'_>> for Element<'a, Message> {
    fn from(title: WindowTitle<'_>) -> Self {
        rich_text(title.spans())
            .size(title.size)
            .line_height(LINE_HEIGHT)
            .wrapping(title.wrapping)
            .into()
    }
}
