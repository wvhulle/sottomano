use iced::{
    Alignment, Border, Color, Element, Length,
    widget::{Column, Space, container, row},
};
use iced_font_awesome::fa_icon_solid;

use crate::{
    bar_program::Message,
    data_sources::{BarWidget, Level},
    user_config::{Config, font},
    widgets::text::bar_text,
};

/// A source-backed leaf: Font Awesome icon + thin vertical gauge (or a text
/// label for a label-only source), tinted by severity. `None` when the source
/// is unavailable.
pub fn source_reading<'a>(
    config: &Config,
    widget: Option<&dyn BarWidget>,
) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let color = level_color(config, widget.level());
    let icon = fa_icon_solid(widget.icon()).size(font::size()).color(color);

    match widget.value() {
        Some(value) => Some(
            row![icon, gauge(value.percent(), color)]
                .spacing(config.layout.horizontal_widget_spacing)
                .align_y(Alignment::Center)
                .into(),
        ),
        None => match widget.label() {
            Some(label) if !label.is_empty() => Some(
                row![icon, bar_text(label).color(color)]
                    .spacing(config.layout.horizontal_widget_spacing)
                    .align_y(Alignment::Center)
                    .into(),
            ),
            _ => Some(icon.into()),
        },
    }
}

/// odin's `PercentGauge`: a thin vertical fill bar for a 0..100 value. A faint
/// full-height track keeps the frame visible; a solid bar fills from the bottom
/// in proportion. Stretches with `Length::Fill` to match the parent height
/// (typically the icon's row), no hardcoded gauge height.
fn gauge<'a>(percent: i64, color: Color) -> Element<'a, Message> {
    let frac = (percent as f32 / 100.0).clamp(0.0, 1.0);
    let total: u16 = 1000;
    let fill_portions: u16 = (frac * f32::from(total)).round() as u16;
    let track_portions: u16 = total - fill_portions;

    let bar = |portions: u16, alpha: f32| {
        container(Space::new())
            .width(3.0)
            .height(Length::FillPortion(portions))
            .style(move |_| container::Style {
                background: Some(Color { a: alpha, ..color }.into()),
                border: Border::default().rounded(1.0),
                ..Default::default()
            })
    };

    Column::with_children([
        bar(track_portions, 0.27).into(),
        bar(fill_portions, 1.0).into(),
    ])
    .width(3.0)
    .height(Length::Fill)
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
