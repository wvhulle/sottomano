//! A reusable inline slider: a Font Awesome icon beside a styled `slider`,
//! filled with the palette's `focus` accent. Extracted from the panel-settings
//! group so every write-capable group (audio, brightness, idle timeouts) drives
//! the same look. The caller supplies the current value and an `on_change`
//! mapping to its own message; throttling the resulting writes is the caller's
//! concern.

use std::ops::RangeInclusive;

use iced::{
    Alignment, Border, Color, Element,
    widget::{Row, slider},
};

use crate::{config::resolved::ResolvedConfig, widgets::icon::Icon};

pub struct Knob {
    pub icon: Icon,
    pub range: RangeInclusive<f32>,
    pub step: f32,
}

impl Knob {
    pub fn view<'a, Message: Clone + 'a>(
        self,
        config: &ResolvedConfig,
        value: f32,
        on_change: impl Fn(f32) -> Message + 'a,
    ) -> Element<'a, Message> {
        let icon = self
            .icon
            .solid()
            .size(config.font.icon_size_px)
            .color(config.palette.bar_foreground);
        // Fill with the palette's `focus` accent rather than iced's default
        // bright-blue primary, which clashes with the bar's gruvbox tones.
        let accent = config.palette.focus;
        let track = config.palette.bar_foreground.scale_alpha(0.25);
        Row::with_children([
            icon.into(),
            slider(self.range, value, on_change)
                .step(self.step)
                .width(config.layout.slider_width_px)
                .style(move |_theme, status| {
                    let fill = match status {
                        slider::Status::Dragged => accent.scale_alpha(0.8),
                        _ => accent,
                    };
                    slider::Style {
                        rail: slider::Rail {
                            backgrounds: (fill.into(), track.into()),
                            width: 4.0,
                            border: Border {
                                radius: 2.0.into(),
                                width: 0.0,
                                color: Color::TRANSPARENT,
                            },
                        },
                        handle: slider::Handle {
                            shape: slider::HandleShape::Circle { radius: 7.0 },
                            background: fill.into(),
                            border_color: Color::TRANSPARENT,
                            border_width: 0.0,
                        },
                    }
                })
                .into(),
        ])
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center)
        .into()
    }
}
