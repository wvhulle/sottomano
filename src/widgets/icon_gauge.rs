use iced::{
    Alignment, Border, Color, Element,
    widget::{Column, Row, Space, container, row, svg, text},
};

use crate::{
    config::resolved::ResolvedConfig,
    data::source::{BarWidget, Glyph, level_color},
    widgets::text::bar_text,
};

pub fn source_reading<'a, Message: 'a>(
    config: &ResolvedConfig,
    widget: Option<&dyn BarWidget>,
    alpha: f32,
) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let color = if widget.disabled() {
        config.palette.inactive
    } else {
        level_color(config, widget.level())
    }
    .scale_alpha(alpha);
    let icon: Element<Message> = match widget.icon() {
        Glyph::SimpleIcon(mark) => svg(mark.handle())
            .width(config.font.icon_size_px)
            .height(config.font.icon_size_px)
            .style(move |_, _| svg::Style { color: Some(color) })
            .into(),
        Glyph::FontAwesome(icon) => icon
            .widget()
            .size(config.font.icon_size_px)
            .color(color)
            .into(),
    };

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
            _ => icon,
        },
    };

    Some(badged(config, reading, widget.badge(), color))
}

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

fn gauge<'a, Message: 'a>(percent: i64, color: Color, height: f32) -> Element<'a, Message> {
    let fill_height = height * (percent as f32 / 100.0).clamp(0.0, 1.0);
    let track_height = height - fill_height;

    let bar = |bar_height: f32, alpha: f32| {
        container(Space::new())
            .width(3.0)
            .height(bar_height)
            .style(move |_| container::Style {
                background: Some(color.scale_alpha(alpha).into()),
                border: Border::default().rounded(1.0),
                ..Default::default()
            })
    };

    Column::with_children([bar(track_height, 0.27).into(), bar(fill_height, 1.0).into()])
        .width(3.0)
        .height(height)
        .into()
}
