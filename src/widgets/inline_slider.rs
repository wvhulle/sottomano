//! odin's `InlineSlider`: a styled horizontal slider filled with the palette's
//! `focus` accent, shared by the write-capable groups (audio, idle) that unfold
//! a slider on click. The caller supplies the range, value, and message
//! mapping; committing the write on release is the caller's concern.

use std::ops::RangeInclusive;

use iced::{
    Border, Color,
    widget::{Slider, slider},
};

use crate::config::resolved::ResolvedConfig;

/// A `focus`-accented slider over `range`, stepping by `step`, emitting
/// `on_change` while dragging and `on_release` when let go.
pub fn inline_slider<'a, Message: Clone + 'a>(
    config: &ResolvedConfig,
    range: RangeInclusive<f32>,
    value: f32,
    step: f32,
    on_change: impl Fn(f32) -> Message + 'a,
    on_release: Message,
) -> Slider<'a, f32, Message> {
    let accent = config.palette.focus;
    let track = config.palette.bar_foreground.scale_alpha(0.25);
    slider(range, value, on_change)
        .step(step)
        .on_release(on_release)
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
}
