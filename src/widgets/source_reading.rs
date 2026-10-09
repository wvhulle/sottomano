use iced::{
    Alignment, Color, Element,
    widget::{Row, row, text},
};

use crate::{
    config::Config,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::Gauge,
        phosphor::{Phosphor, Weight},
        text::bar_text,
        widget_state::WidgetState,
    },
};

pub fn source_reading<'a, Message: 'a>(
    config: &Config,
    widget: Option<&dyn BarWidget>,
    opacity: f32,
) -> Option<Element<'a, Message>> {
    Some(composed_reading(config, widget?, opacity, reading_value))
}

pub fn labeled_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    opacity: f32,
) -> Element<'a, Message> {
    composed_reading(config, widget, opacity, label_value)
}

fn composed_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    opacity: f32,
    value: fn(&Config, &dyn BarWidget, Color) -> Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let color = reading_color(config, widget).scale_alpha(opacity);
    let icon = widget.icon().view(config.font.icon_size_px, color);

    let reading = match value(config, widget, color) {
        Some(value) => row![icon, value]
            .spacing(icon_value_gap(config))
            .align_y(Alignment::Center)
            .into(),
        None => icon,
    };

    if widget.state() == WidgetState::Failed {
        return reading;
    }
    badged(config, reading, widget.badge(), color)
}

pub fn icon_value_gap(config: &Config) -> f32 {
    config.spacing.widget_spacing_px / 2.0
}

pub fn reading_color(config: &Config, widget: &dyn BarWidget) -> Color {
    widget.state().color(&config.palette)
}

pub fn reading_value<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    color: Color,
) -> Option<Element<'a, Message>> {
    if widget.state() == WidgetState::Failed {
        return Some(failure_marker(config, color));
    }
    match widget.gauge() {
        Some(fill) => Some(
            Gauge::new(fill, color, config.font.icon_size_px)
                .live(widget.live_gauge().unwrap_or(fill))
                .into(),
        ),
        None => label_text(config, widget, color),
    }
}

fn label_value<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    color: Color,
) -> Option<Element<'a, Message>> {
    if widget.state() == WidgetState::Failed {
        return Some(failure_marker(config, color));
    }
    label_text(config, widget, color)
}

fn label_text<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    color: Color,
) -> Option<Element<'a, Message>> {
    widget
        .localized_label(config.locale)
        .filter(|label| !label.is_empty())
        .map(|label| bar_text(&config.font, label).color(color).into())
}

fn failure_marker<'a, Message: 'a>(config: &Config, color: Color) -> Element<'a, Message> {
    Glyph::from(Phosphor::Warning(Weight::Thin)).view(config.font.icon_size_px, color)
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
                .size(config.font.small_text_size_px)
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
