use iced::{
    Color, Element, Theme,
    widget::{Column, Row, Text, text, text::Wrapping},
};
use sottomano_desktop_sources::desktop_windows_apps::window_title_labels::WindowLabel;
use sottomano_toml_config::Config;

use crate::widgets::{
    configured_theme::{secondary_text_color, text_color},
    text::{LINE_HEIGHT, font_faces_of},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocationPlacement {
    Hidden,
    BesideTitle,
    AboveTitle,
}

pub struct WindowTitle<'a> {
    label: &'a WindowLabel,
    config: &'a Config,
    size: f32,
    opacity: f32,
    wrapping: Wrapping,
    location_placement: LocationPlacement,
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
            location_placement: LocationPlacement::BesideTitle,
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
            location_placement: LocationPlacement::Hidden,
            ..self
        }
    }

    #[must_use]
    pub fn location_above_title(self) -> Self {
        Self {
            location_placement: LocationPlacement::AboveTitle,
            ..self
        }
    }

    #[must_use]
    pub fn color(self, color: fn(&Theme) -> Color) -> Self {
        Self { color, ..self }
    }

    fn styled_text<'b>(&self, content: &str, color: fn(&Theme) -> Color) -> Text<'b> {
        let opacity = self.opacity;
        text(content.to_owned())
            .font(font_faces_of(&self.config.theme.font).regular)
            .size(self.size)
            .line_height(LINE_HEIGHT)
            .wrapping(self.wrapping)
            .style(move |theme: &Theme| text::Style {
                color: Some(color(theme).scale_alpha(opacity)),
            })
    }
}

impl<'a, Message: 'a> From<WindowTitle<'_>> for Element<'a, Message> {
    fn from(title: WindowTitle<'_>) -> Self {
        let own_text = title.styled_text(title.label.own_text(), title.color);
        let Some(location) = title.label.location_text() else {
            return own_text.into();
        };
        let location_text = title.styled_text(location, secondary_text_color);
        let spacing = title.config.theme.spacing.item_gap;
        match title.location_placement {
            LocationPlacement::Hidden => own_text.into(),
            LocationPlacement::BesideTitle => Row::new()
                .push(location_text)
                .push(own_text)
                .spacing(spacing)
                .into(),
            LocationPlacement::AboveTitle => Column::new()
                .push(location_text)
                .push(own_text)
                .spacing(spacing)
                .into(),
        }
    }
}
