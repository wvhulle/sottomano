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
        svg_icon::PHOSPHOR_INK_FRACTION,
        text::bar_text,
        widget_state::WidgetState,
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
            .spacing(icon_value_gap(config))
            .align_y(Alignment::Center)
            .into(),
        None => icon,
    };

    if widget.state() == WidgetState::Failed {
        return Some(reading);
    }
    Some(badged(config, reading, widget.badge(), color))
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
        return Some(
            Glyph::from(Phosphor::Warning(Weight::Thin)).view(config.font.icon_size_px, color),
        );
    }
    match widget.gauge() {
        Some(fill) => Some(
            Gauge::new(
                fill,
                color,
                config.font.icon_size_px * PHOSPHOR_INK_FRACTION,
            )
            .live(widget.live_gauge().unwrap_or(fill))
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
