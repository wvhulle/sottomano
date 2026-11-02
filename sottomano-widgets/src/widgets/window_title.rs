use iced::{
    Color, Element, Font, Theme, border,
    widget::{
        rich_text, span,
        text::{self, Span, Wrapping},
    },
};
use sottomano_config::Config;
use sottomano_sources::desktop_windows_apps::window_title_labels::WindowLabel;

use crate::widgets::{
    configured_theme::text_color,
    text::{FontFaces, LINE_HEIGHT},
};

const CHIP_TEXT_GAP: &str = " ";

pub struct WindowTitle<'a> {
    label: &'a WindowLabel,
    config: &'a Config,
    size: f32,
    opacity: f32,
    wrapping: Wrapping,
    shows_location: bool,
    color: fn(&Theme) -> Color,
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
            shows_location: true,
            color: text_color,
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

    #[must_use]
    pub fn without_location(self) -> Self {
        Self {
            shows_location: false,
            ..self
        }
    }

    #[must_use]
    pub fn color(self, color: fn(&Theme) -> Color) -> Self {
        Self { color, ..self }
    }

    fn regular_span(&self, content: &str) -> Span<'static> {
        styled_span(content, FontFaces::from(&self.config.theme.font).regular)
    }

    fn chip_span(&self, location: &str) -> Span<'static> {
        let chip_background = Color::from(self.config.theme.palette.background_strong.components)
            .scale_alpha(self.opacity);
        self.regular_span(&format!(" {location} "))
            .background(chip_background)
            .border(border::rounded(self.size / 4.0))
    }

    fn spans(&self) -> Vec<Span<'static>> {
        let own_text = self.regular_span(self.label.own_text());
        match self.label.location_text().filter(|_| self.shows_location) {
            Some(location) => vec![
                self.chip_span(location),
                self.regular_span(CHIP_TEXT_GAP),
                own_text,
            ],
            None => vec![own_text],
        }
    }
}

fn styled_span(content: &str, font: Font) -> Span<'static> {
    span(content.to_owned()).font(font)
}

impl<'a, Message: 'a> From<WindowTitle<'_>> for Element<'a, Message> {
    fn from(title: WindowTitle<'_>) -> Self {
        let WindowTitle { color, opacity, .. } = title;
        rich_text(title.spans())
            .style(move |theme: &Theme| text::Style {
                color: Some(color(theme).scale_alpha(opacity)),
            })
            .size(title.size)
            .line_height(LINE_HEIGHT)
            .wrapping(title.wrapping)
            .into()
    }
}
