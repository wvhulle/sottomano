use iced::{
    Alignment, Border, Color, Element,
    widget::{Column, Space, container, row},
};

use crate::{
    bar_program::Message,
    data_sources::{BarWidget, Level},
    user_config::Config,
    widgets::text::bar_text,
};

/// A source-backed leaf: Font Awesome icon + thin vertical gauge (or a text
/// label for a label-only source), tinted by severity. `None` when the source
/// is unavailable. `alpha` scales every colour's opacity so a reading can fade
/// in/out (1.0 = fully opaque).
pub fn source_reading<'a>(
    config: &Config,
    widget: Option<&dyn BarWidget>,
    alpha: f32,
) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let color = level_color(config, widget.level()).scale_alpha(alpha);
    let icon = widget
        .icon()
        .solid()
        .size(config.font.icon_size_px)
        .color(color);

    match widget.value() {
        Some(value) => Some(
            row![
                icon,
                gauge(value.percent(), color, config.font.icon_size_px)
            ]
            .spacing(config.layout.horizontal_widget_spacing_px)
            .align_y(Alignment::Center)
            .into(),
        ),
        None => match widget.label() {
            Some(label) if !label.is_empty() => Some(
                row![icon, bar_text(label).color(color)]
                    .spacing(config.layout.horizontal_widget_spacing_px)
                    .align_y(Alignment::Center)
                    .into(),
            ),
            _ => Some(icon.into()),
        },
    }
}

/// odin's `PercentGauge`: a thin vertical fill bar for a 0..100 value. A faint
/// full-height track keeps the frame visible; a solid bar fills from the bottom
/// in proportion. Height matches the icon size so it lines up with the
/// Font Awesome icon beside it.
fn gauge<'a>(percent: i64, color: Color, height: f32) -> Element<'a, Message> {
    let fill_height = height * (percent as f32 / 100.0).clamp(0.0, 1.0);
    let track_height = height - fill_height;

    let bar = |bar_height: f32, alpha: f32| {
        container(Space::new())
            .width(3.0)
            .height(bar_height)
            .style(move |_| container::Style {
                background: Some(
                    Color {
                        a: alpha * color.a,
                        ..color
                    }
                    .into(),
                ),
                border: Border::default().rounded(1.0),
                ..Default::default()
            })
    };

    Column::with_children([bar(track_height, 0.27).into(), bar(fill_height, 1.0).into()])
        .width(3.0)
        .height(height)
        .into()
}

/// Severity → foreground, mirroring odin's per-level tints: ok stays the
/// palette's bar foreground; warning/critical take the palette's
/// warning/urgent tints.
fn level_color(config: &Config, level: Level) -> Color {
    match level {
        Level::Ok => config.palette.bar_foreground(),
        Level::Warning => config.palette.warning(),
        Level::Critical => config.palette.urgent(),
    }
}
