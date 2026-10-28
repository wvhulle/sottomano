use iced::{
    Alignment, Color, Element,
    widget::{Row, row, text},
};
use sottomano_config::Config;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::Gauge,
        palette_colors::PaletteColors,
        phosphor::{Phosphor, Weight},
        steady_label_width::{LabelShape, steady_label_width},
        text::{FontFaces, bar_text},
        widget_state::WidgetState,
    },
};

pub fn source_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    opacity: f32,
    clock: Clock,
) -> Element<'a, Message> {
    composed_reading(config, widget, opacity, clock, reading_value)
}

pub fn labeled_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    opacity: f32,
    clock: Clock,
) -> Element<'a, Message> {
    composed_reading(config, widget, opacity, clock, label_value)
}

fn composed_reading<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    opacity: f32,
    clock: Clock,
    value: fn(&Config, &dyn BarWidget, Color, Clock) -> Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let color = reading_color(config, widget, clock).scale_alpha(opacity);
    let bare_icon = widget
        .icon_at(clock)
        .view(config.theme.font.icon_size_px, color);
    let icon = match widget.identity_ordinal() {
        Some(ordinal) => superscripted(
            config,
            bare_icon,
            ordinal.to_string(),
            PaletteColors::from(&config.theme.palette)
                .inactive
                .scale_alpha(opacity),
        ),
        None => bare_icon,
    };

    let reading = match value(config, widget, color, clock) {
        Some(value) => row![icon, value]
            .spacing(icon_value_gap(config))
            .align_y(Alignment::Center)
            .into(),
        None => icon,
    };

    if widget.state(clock) == WidgetState::Failed {
        return reading;
    }
    badged(config, reading, widget.badge(), color)
}

#[must_use]
pub fn icon_value_gap(config: &Config) -> f32 {
    config.layout.panel.widget_spacing / 2.0
}

pub fn reading_color(config: &Config, widget: &dyn BarWidget, clock: Clock) -> Color {
    widget
        .state(clock)
        .color(&PaletteColors::from(&config.theme.palette))
}

pub fn reading_value<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    color: Color,
    clock: Clock,
) -> Option<Element<'a, Message>> {
    if widget.state(clock) == WidgetState::Failed {
        return Some(failure_marker(config, color));
    }
    match widget.gauge() {
        Some(fill) => Some(
            Gauge::new(fill, color, config.theme.font.icon_size_px)
                .live(widget.live_gauge().unwrap_or(fill))
                .into(),
        ),
        None => label_text(config, widget, color, clock),
    }
}

fn label_value<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    color: Color,
    clock: Clock,
) -> Option<Element<'a, Message>> {
    if widget.state(clock) == WidgetState::Failed {
        return Some(failure_marker(config, color));
    }
    label_text(config, widget, color, clock)
}

fn label_text<'a, Message: 'a>(
    config: &Config,
    widget: &dyn BarWidget,
    color: Color,
    clock: Clock,
) -> Option<Element<'a, Message>> {
    widget
        .label(clock, config.calendar.locale)
        .filter(|label| !label.is_empty())
        .map(|label| {
            let shape = LabelShape {
                widget_name: widget.name(),
                character_count: label.chars().count(),
            };
            steady_label_width(
                shape,
                bar_text(&config.theme.font, label).color(color).into(),
            )
        })
}

fn failure_marker<'a, Message: 'a>(config: &Config, color: Color) -> Element<'a, Message> {
    Glyph::from(Phosphor::Warning(Weight::Thin)).view(config.theme.font.icon_size_px, color)
}

fn badged<'a, Message: 'a>(
    config: &Config,
    reading: Element<'a, Message>,
    badge: Option<u32>,
    color: Color,
) -> Element<'a, Message> {
    match badge {
        Some(count) if count > 0 => superscripted(config, reading, count.to_string(), color),
        _ => reading,
    }
}

fn superscripted<'a, Message: 'a>(
    config: &Config,
    marked: Element<'a, Message>,
    digits: String,
    color: Color,
) -> Element<'a, Message> {
    let superscript = text(digits)
        .size(config.theme.font.small_text_size_px)
        .font(FontFaces::from(&config.theme.font).regular)
        .color(color);
    Row::with_children([marked, superscript.into()])
        .spacing(1.0)
        .align_y(Alignment::Start)
        .into()
}
