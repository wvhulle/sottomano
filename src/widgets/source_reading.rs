use iced::{
    Alignment, Color, Element,
    widget::{Row, row, text},
};

use crate::{
    config::Config,
    widgets::{
        bar_widget::{BarWidget, level_color},
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
    let color = reading_color(config, widget).scale_alpha(opacity);
    let icon = widget.icon().view(config.font.icon_size_px, color);

    let reading = match reading_value(config, widget, color) {
        Some(value) => row![icon, value]
            .spacing(config.spacing.widget_spacing_px)
            .align_y(Alignment::Center)
            .into(),
        None => icon,
    };

    if widget.failed() {
        return Some(reading);
    }
    Some(badged(config, reading, widget.badge(), color))
}

pub fn reading_color(config: &Config, widget: &dyn BarWidget) -> Color {
    if widget.disabled() {
        config.palette.inactive
    } else {
        level_color(&config.palette, widget.level())
    }
}

pub fn reading_value<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    color: Color,
) -> Option<Element<'a, Message>> {
    if widget.failed() {
        return Some(Solid::TriangleExclamation.view(config.font.icon_size_px, color));
    }
    match widget.value() {
        Some(value) => Some(
            Gauge::new(value.percent(), color, config.font.icon_size_px)
                .live(widget.live_fraction())
                .into(),
        ),
        None => widget
            .label()
            .filter(|label| !label.is_empty())
            .map(|label| bar_text(&config.font, label).color(color).into()),
    }
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
