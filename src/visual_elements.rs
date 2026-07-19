//! Shared rendering primitives (odin's `ReusableComponent` equivalent): the visual
//! vocabulary every leaf widget and concrete widget is drawn with — a gauge fill, a
//! state indicator, the edge-anchored layout it sits in, and the source-backed leaf
//! rendering that combines them. Not a widget itself; this is what widgets are drawn
//! with, so it lives beside `bar_layout` rather than inside it. Each function takes
//! exactly the data it needs (`&Config`, `Position`) rather than a shared render context.

use iced::alignment::Vertical;
use iced::widget::text::Shaping;
use iced::widget::{container, row, stack, text};
use iced::{Alignment, Border, Color, Element, Font, Length};
use iced_font_awesome::fa_icon_solid;

use crate::bar_program::Message;
use crate::data_sources::{BarWidget, Level};
use crate::layer_surface::Position;
use crate::user_config::Config;

/// Coerce a concrete source reference to `&dyn BarWidget` for uniform rendering.
pub fn as_bar_widget<W: BarWidget>(w: &W) -> &dyn BarWidget {
    w
}

/// A pager/menu slot: `content` centred over the bar height, with an `inner` state bar
/// (a hairline) pinned to the inner screen edge and an optional `outer` row (the workspace
/// window-tick segments) pinned to the outer edge. The shared layout for menu affordances,
/// pager focus indicators, and the workspace window indicators — odin's `Hairline` +
/// `SlotHighlight` + `WorkspaceWindowIndicators` composition.
///
/// A `Stack` (overlay, not an in-flow `Column`) so the bars sit *on* the bar's own
/// vertical padding rather than adding height — the slot stays the bar's height, `content`
/// centred, bars at the edges. The `Stack` sizes to its base layer (`content`), so the
/// Fill-width bars are constrained to the content width and the slot is exactly as wide as
/// its label. Inner edge = top on a bottom bar, bottom on a top bar, so a bar always
/// points into the screen.
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
    let edge_layer = |element, edge| {
        container(element)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([0.0, 3.0])
            .align_y(edge)
    };
    let base = container(content).height(Length::Fill).align_y(Vertical::Center);
    let mut layers: Vec<Element<'a, Message>> = vec![base.into(), edge_layer(inner, inner_edge).into()];
    if let Some(outer) = outer {
        layers.push(edge_layer(outer, outer_edge).into());
    }
    iced::widget::Stack::with_children(layers).into()
}

/// A source-backed leaf: Font Awesome icon + thin vertical gauge (or a text label for a
/// label-only source), tinted by severity. `None` when the source is unavailable.
pub fn leaf<'a>(config: &Config, widget: Option<&dyn BarWidget>) -> Option<Element<'a, Message>> {
    let widget = widget?;
    let size = config.font.size as f32;
    let color = level_color(config, widget.level());
    let icon = fa_icon_solid(widget.icon()).size(size).color(color);

    match widget.value() {
        Some(value) => Some(
            row![icon, gauge(value.percent(), color, size)]
                .spacing(config.layout.horizontal_widget_spacing as f32)
                .align_y(Alignment::Center)
                .into(),
        ),
        None => match widget.label() {
            Some(label) if !label.is_empty() => Some(
                row![icon, text(label).size(size).color(color).shaping(Shaping::Advanced)]
                    .spacing(config.layout.horizontal_widget_spacing as f32)
                    .align_y(Alignment::Center)
                    .into(),
            ),
            _ => Some(icon.into()),
        },
    }
}

/// odin's `PercentGauge`: a thin vertical fill bar for a 0..100 value. A faint full-height
/// track keeps the frame visible; a solid bar fills from the bottom in proportion.
pub fn gauge<'a>(percent: i64, color: Color, font_size: f32) -> Element<'a, Message> {
    let frac = (percent as f32 / 100.0).clamp(0.0, 1.0);
    let fill_height = (font_size * frac).max(1.0);
    let bar = |height: f32, alpha: f32| {
        container(iced::widget::Space::new())
            .width(3.0)
            .height(height)
            .style(move |_| container::Style {
                background: Some(Color { a: alpha, ..color }.into()),
                border: Border::default().rounded(1.0),
                ..Default::default()
            })
    };
    // Fill sits at the bottom of the track's band; the outer box adds odin's +3 padding.
    let track = bar(font_size, 0.27);
    let fill = container(bar(fill_height, 1.0))
        .height(font_size)
        .align_y(Vertical::Bottom);
    container(stack![track, fill])
        .height(font_size + 3.0)
        .align_y(Vertical::Center)
        .into()
}

/// odin's `Hairline`: a thin rounded bar, the shared state-indicator primitive for the
/// menu-group expand affordance, the pager focus/urgent bars, and the per-window tick
/// segments — each just picks a `color`, `opacity`, and `thickness`. Fill-width (the
/// caller's slot sizes it); the caller pins it to an edge via `edge_slot`.
pub fn hairline<'a>(color: Color, opacity: f32, thickness: f32) -> Element<'a, Message> {
    container(iced::widget::Space::new().width(Length::Fill))
        .width(Length::Fill)
        .height(thickness)
        .style(move |_| container::Style {
            background: Some(Color { a: opacity, ..color }.into()),
            border: Border::default().rounded(thickness),
            ..Default::default()
        })
        .into()
}

/// Map an odin font-family string to an iced `Font`. odin's `sans-serif`/`monospace` are
/// fontconfig generic aliases; Qt resolves them to concrete system fonts (here IBM Plex
/// Sans / Lilex Nerd Font Mono) and renders those. iced loads system fonts too, so we
/// resolve the aliases to the same concrete families to match odin's rendering rather
/// than falling back to iced's built-in Fira Sans. TODO: resolve the alias via
/// fontconfig at startup instead of hardcoding the current machine's default.
pub fn font_family(family: &str) -> Font {
    match family {
        "sans-serif" | "" => Font::with_name("IBM Plex Sans"),
        "monospace" => Font::with_name("Lilex Nerd Font Mono"),
        name => Font::with_name(Box::leak(name.to_string().into_boxed_str())),
    }
}

/// Severity → foreground, mirroring odin's per-level tints: ok stays the white bar
/// foreground; warning/critical take the palette's brightness/critical tints.
pub fn level_color(config: &Config, level: Level) -> Color {
    match level {
        Level::Ok => Color::WHITE,
        Level::Warning => config.palette.brightness_tint.0,
        Level::Critical => config.palette.critical_tint.0,
    }
}
