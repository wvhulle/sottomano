use corredo::theming::{LINE_HEIGHT, secondary_text_color, text_color};
use iced::{
    Color, Element, Theme,
    widget::{Column, Row, Text, text, text::Wrapping},
};
use sottomano_desktop_sources::desktop_windows_apps::window_title_labels::WindowLabel;
use sottomano_toml_config::Config;

use crate::widgets::text::font_faces_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupKeyPlacement {
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
    group_key_placement: GroupKeyPlacement,
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
            group_key_placement: GroupKeyPlacement::BesideTitle,
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
    pub fn without_group_key(self) -> Self {
        Self {
            group_key_placement: GroupKeyPlacement::Hidden,
            ..self
        }
    }

    #[must_use]
    pub fn group_key_above_title(self) -> Self {
        Self {
            group_key_placement: GroupKeyPlacement::AboveTitle,
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
        let subject_text = title.styled_text(title.label.subject_text(), title.color);
        let Some(group_key) = title.label.group_key() else {
            return subject_text.into();
        };
        let group_key_text = title.styled_text(group_key.heading_text(), secondary_text_color);
        let spacing = title.config.theme.spacing.item_gap;
        match title.group_key_placement {
            GroupKeyPlacement::Hidden => subject_text.into(),
            GroupKeyPlacement::BesideTitle => Row::new()
                .push(group_key_text)
                .push(subject_text)
                .spacing(spacing)
                .into(),
            GroupKeyPlacement::AboveTitle => Column::new()
                .push(group_key_text)
                .push(subject_text)
                .spacing(spacing)
                .into(),
        }
    }
}
