use std::iter;

use iced::{
    Element, Font,
    widget::{
        rich_text, span,
        text::{Span, Wrapping},
    },
};
use vetrata_config::Config;
use vetrata_sources::window_label::WindowLabel;

use crate::widgets::{
    palette_colors::PaletteColors,
    text::{FontFaces, LINE_HEIGHT},
};

const SEPARATOR: &str = " · ";

pub struct WindowTitle<'a> {
    label: &'a WindowLabel,
    config: &'a Config,
    size: f32,
    opacity: f32,
    wrapping: Wrapping,
}

impl<'a> WindowTitle<'a> {
    #[must_use]
    pub fn new(label: &'a WindowLabel, config: &'a Config) -> Self {
        Self {
            label,
            config,
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

    fn main_span(&self, content: &str) -> Span<'static> {
        self.styled(content, FontFaces::from(&self.config.theme.font).regular)
    }

    fn context_span(&self, content: &str) -> Span<'static> {
        self.styled(content, FontFaces::from(&self.config.theme.font).italic)
    }

    fn separator_span(&self) -> Span<'static> {
        self.styled(SEPARATOR, FontFaces::from(&self.config.theme.font).regular)
    }

    fn styled(&self, content: &str, font: Font) -> Span<'static> {
        span(content.to_owned()).font(font).color(
            PaletteColors::from(&self.config.theme.palette)
                .bar_foreground
                .scale_alpha(self.opacity),
        )
    }

    fn spans(&self) -> Vec<Span<'static>> {
        match self.label {
            WindowLabel::KittyAgent(kitty) => {
                iter::once(self.main_span(kitty.working_location.display_text()))
                    .chain(kitty.foreground_activity.iter().flat_map(|activity| {
                        [
                            self.separator_span(),
                            self.context_span(activity.display_text()),
                        ]
                    }))
                    .collect()
            }
            WindowLabel::Firefox { page, .. } | WindowLabel::BrandedBrowser(page) => page
                .site
                .iter()
                .flat_map(|site| [self.context_span(site), self.separator_span()])
                .chain(iter::once(self.main_span(&page.page)))
                .collect(),
            WindowLabel::Titled(title) => vec![self.main_span(title)],
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
