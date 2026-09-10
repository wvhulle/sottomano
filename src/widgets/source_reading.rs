use iced::{
    Alignment, Color, Element,
    widget::{Row, row, svg, text},
};

use crate::{
    config::Config,
    widgets::{
        bar_widget::{BarWidget, Glyph, level_color},
        font_awesome::Solid,
        gauge::Gauge,
        text::bar_text,
    },
};

pub fn source_reading<'a, Message: 'a>(
    config: &Config,
    widget: Option<&dyn BarWidget>,
    opacity: f32,
) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let color = if widget.disabled() {
        config.palette.inactive
    } else {
        level_color(config, widget.level())
    }
    .scale_alpha(opacity);
    let icon: Element<Message> = match widget.icon() {
        Glyph::SimpleIcon(mark) => svg(mark.handle())
            .width(config.font.icon_size_px)
            .height(config.font.icon_size_px)
            .style(move |_, _| svg::Style { color: Some(color) })
            .into(),
        Glyph::FontAwesome(icon) => icon
            .glyph()
            .size(config.font.icon_size_px)
            .color(color)
            .into(),
    };

    if widget.failed() {
        let warning = Solid::TriangleExclamation
            .glyph()
            .size(config.font.icon_size_px)
            .color(color);
        return Some(
            row![icon, warning]
                .spacing(config.spacing.widget_spacing_px)
                .align_y(Alignment::Center)
                .into(),
        );
    }

    let reading = match widget.value() {
        Some(value) => row![
            icon,
            Gauge::new(value.percent(), color, config.font.icon_size_px)
                .live(widget.live_fraction())
        ]
        .spacing(config.spacing.widget_spacing_px)
        .align_y(Alignment::Center)
        .into(),
        None => match widget.label() {
            Some(label) if !label.is_empty() => {
                row![icon, bar_text(&config.font, label).color(color)]
                    .spacing(config.spacing.widget_spacing_px)
                    .align_y(Alignment::Center)
                    .into()
            }
            _ => icon,
        },
    };

    Some(badged(config, reading, widget.badge(), color))
}

fn badged<'a, Message: 'a>(
    config: &Config,
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
