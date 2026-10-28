use std::iter;

use iced::{
    Element, Font,
    widget::{
        rich_text, span,
        text::{Span, Wrapping},
    },
};
use sottomano_config::Config;
use sottomano_sources::desktop_windows_apps::window_title_labels::WindowLabel;

use crate::widgets::{
    text::{FontFaces, LINE_HEIGHT},
    widget_state::{StateTint, WidgetState},
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
        styled_span(content, FontFaces::from(&self.config.theme.font).regular)
    }

    fn context_span(&self, content: &str) -> Span<'static> {
        styled_span(content, FontFaces::from(&self.config.theme.font).italic)
    }

    fn separator_span(&self) -> Span<'static> {
        styled_span(SEPARATOR, FontFaces::from(&self.config.theme.font).regular)
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

fn styled_span(content: &str, font: Font) -> Span<'static> {
    span(content.to_owned()).font(font)
}

impl<'a, Message: 'a> From<WindowTitle<'_>> for Element<'a, Message> {
    fn from(title: WindowTitle<'_>) -> Self {
        let tint = StateTint {
            state: WidgetState::Idle,
            opacity: title.opacity,
        };
        rich_text(title.spans())
            .style(tint.text_style())
            .size(title.size)
            .line_height(LINE_HEIGHT)
            .wrapping(title.wrapping)
            .into()
    }
}
