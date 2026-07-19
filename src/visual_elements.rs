//! Shared rendering primitives (odin's `ReusableComponent` equivalent): the
//! visual vocabulary every leaf widget and concrete widget is drawn with — a
//! gauge fill, a state indicator, the edge-anchored layout it sits in, and the
//! source-backed leaf rendering that combines them. Not a widget itself; this
//! is what widgets are drawn with, so it lives beside `bar_layout` rather than
//! inside it. Each function takes exactly the data it needs (`&Config`,
//! `Position`) rather than a shared render context.

use iced::{
    Alignment, Border, Color, Element, Length,
    alignment::Vertical,
    widget::{Column, Space, container, row, text, text::Shaping},
};
use iced_font_awesome::fa_icon_solid;

use crate::{
    bar_program::Message,
    data_sources::{BarWidget, Level},
    layer_surface::Position,
    user_config::Config,
};

/// A pager/menu slot: `content` centred over the bar height, with an `inner`
/// state bar (a hairline) pinned to the inner screen edge and an optional
/// `outer` row (the workspace window-tick segments) pinned to the outer edge.
/// The shared layout for menu affordances, pager focus indicators, and the
/// workspace window indicators — odin's `Hairline` + `SlotHighlight` +
/// `WorkspaceWindowIndicators` composition.
///
/// A `Stack` (overlay, not an in-flow `Column`) so the bars sit *on* the bar's
/// own vertical padding rather than adding height — the slot stays the bar's
/// height, `content` centred, bars at the edges. The `Stack` sizes to its base
/// layer (`content`), so the Fill-width bars are constrained to the content
/// width and the slot is exactly as wide as its label. Inner edge = top on a
/// bottom bar, bottom on a top bar, so a bar always points into the screen.
pub fn edge_slot<'a>(
    position: Position,
    content: Element<'a, Message>,
    inner: Element<'a, Message>,
    outer: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let (inner_edge, outer_edge) = match position {
        Position::Bottom => (Vertical::Top, Vertical::Bottom),
        Position::Top => (Vertical::Bottom, Vertical::Top),
    };
    // No horizontal padding: the state bars span the slot edge to edge (odin's
    // `PagerSlot` — "the slot is exactly its content: the state bars span it edge
    // to edge, so any padding here would widen the bars rather than separate
    // them"). Slot separation is the row `spacing`, never inset here.
    let edge_layer = |element, edge| {
        container(element)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(edge)
    };
    let base = container(content)
        .height(Length::Fill)
        .align_y(Vertical::Center);
    let mut layers: Vec<Element<'a, Message>> =
        vec![base.into(), edge_layer(inner, inner_edge).into()];
    if let Some(outer) = outer {
        layers.push(edge_layer(outer, outer_edge).into());
    }
    iced::widget::Stack::with_children(layers).into()
}

/// A source-backed leaf: Font Awesome icon + thin vertical gauge (or a text
/// label for a label-only source), tinted by severity. `None` when the source
/// is unavailable.
pub fn leaf<'a>(config: &Config, widget: Option<&dyn BarWidget>) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let color = level_color(config, widget.level());
    let icon = fa_icon_solid(widget.icon()).color(color);

    match widget.value() {
        Some(value) => Some(
            row![icon, gauge(value.percent(), color)]
                .spacing(config.layout.horizontal_widget_spacing as f32)
                .align_y(Alignment::Center)
                .into(),
        ),
        None => match widget.label() {
            Some(label) if !label.is_empty() => Some(
                row![
                    icon,
                    text(label)
                        .color(color)
                        .shaping(Shaping::Advanced)
                ]
                .spacing(config.layout.horizontal_widget_spacing as f32)
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
pub fn gauge<'a>(percent: i64, color: Color) -> Element<'a, Message> {
    let frac = (percent as f32 / 100.0).clamp(0.0, 1.0);
    let total: u16 = 1000;
    let fill_portions: u16 = (frac * total as f32).round() as u16;
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

/// odin's `Hairline`: a thin rounded bar, the shared state-indicator primitive
/// for the menu-group expand affordance, the pager focus/urgent bars, and the
/// per-window tick segments — each just picks a `color`, `opacity`, and
/// `thickness`. Fill-width (the caller's slot sizes it); the caller pins it to
/// an edge via `edge_slot`.
pub fn hairline<'a>(color: Color, opacity: f32, thickness: f32) -> Element<'a, Message> {
    container(iced::widget::Space::new().width(Length::Fill))
        .width(Length::Fill)
        .height(thickness)
        .style(move |_| container::Style {
            background: Some(
                Color {
                    a: opacity,
                    ..color
                }
                .into(),
            ),
            border: Border::default().rounded(thickness),
            ..Default::default()
        })
        .into()
}

/// Bar foreground: fixed white, mirroring odin's `Config.barText`. The bar sits
/// over a (blurred/shadowed) backdrop rather than a `surface`-colored panel, so
/// it wants a constant light foreground regardless of which palette is active.
pub(crate) const BAR_FOREGROUND: Color = Color::WHITE;

/// Severity → foreground, mirroring odin's per-level tints: ok stays the white
/// bar foreground; warning/critical take the palette's warning/urgent tints.
pub fn level_color(config: &Config, level: Level) -> Color {
    match level {
        Level::Ok => BAR_FOREGROUND,
        Level::Warning => config.palette.warning(),
        Level::Critical => config.palette.urgent(),
    }
}
