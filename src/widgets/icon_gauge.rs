use iced::{
    Alignment, Border, Color, Element,
    widget::{Column, Row, Space, container, row, text},
};

use crate::{
    config::resolved::ResolvedConfig,
    data_sources::{BarWidget, level_color},
    widgets::text::bar_text,
};

/// A source-backed leaf: Font Awesome icon + thin vertical gauge (or a text
/// label for a label-only source), tinted by severity, with an optional
/// superscript badge. `None` when the source is unavailable. `alpha` scales
/// every colour's opacity so a reading can fade in/out (1.0 = fully opaque); a
/// `disabled` reading is dimmed further.
pub fn source_reading<'a, Message: 'a>(
    config: &ResolvedConfig,
    widget: Option<&dyn BarWidget>,
    alpha: f32,
) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let dim = if widget.disabled() { 0.4 } else { 1.0 };
    let color = level_color(config, widget.level()).scale_alpha(alpha * dim);
    let icon = widget
        .icon()
        .solid()
        .size(config.font.icon_size_px)
        .color(color);

    let reading = match widget.value() {
        Some(value) => row![
            icon,
            gauge(value.percent(), color, config.font.icon_size_px)
        ]
        .spacing(config.layout.horizontal_widget_spacing_px)
        .align_y(Alignment::Center)
        .into(),
        None => match widget.label() {
            Some(label) if !label.is_empty() => {
                row![icon, bar_text(&config.font, label).color(color)]
                    .spacing(config.layout.horizontal_widget_spacing_px)
                    .align_y(Alignment::Center)
                    .into()
            }
            _ => Element::from(icon),
        },
    };

    Some(badged(config, reading, widget.badge(), color))
}

/// Attach a superscript count to a reading (odin's `badge`), top-aligned to the
/// right so it rides above the glyph's baseline. `None` or `0` leaves the
/// reading untouched.
fn badged<'a, Message: 'a>(
    config: &ResolvedConfig,
    reading: Element<'a, Message>,
    badge: Option<u32>,
    color: Color,
) -> Element<'a, Message> {
    match badge {
        Some(count) if count > 0 => {
            let superscript = text(count.to_string())
                .size(config.font.text_size_px * 0.62)
                .font(config.font.iced_font)
                .color(color);
            Row::with_children([reading, superscript.into()])
                .spacing(1.0)
                .align_y(Alignment::Start)
                .into()
        }
        _ => reading,
    }
}

/// odin's `PercentGauge`: a thin vertical fill bar for a 0..100 value. A faint
/// full-height track keeps the frame visible; a solid bar fills from the bottom
/// in proportion. Height matches the icon size so it lines up with the
/// Font Awesome icon beside it.
fn gauge<'a, Message: 'a>(percent: i64, color: Color, height: f32) -> Element<'a, Message> {
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
