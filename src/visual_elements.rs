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

/// Gap (px) between a widget and its inner-edge hairline indicator.
const HAIRLINE_GAP: f32 = 3.0;

/// Coerce a concrete source reference to `&dyn BarWidget` for uniform rendering.
pub fn as_bar_widget<W: BarWidget>(w: &W) -> &dyn BarWidget {
    w
}

/// Place a state `indicator` (a hairline) against the inner screen edge of `content` —
/// the shared basis for menu affordances and pager focus/urgent bars. A real in-flow
/// `Column` sibling (not a `Stack` overlay): the indicator's height is unconditionally
/// part of the layout, so it never depends on resolving `Length::Fill` against a
/// `Shrink`-sized ancestor (a real bug: the overlay approach silently failed to paint
/// when this widget's parent row didn't happen to stretch to the bar's full height).
/// The indicator sits below `content` on a top bar, above it on a bottom bar, so it
/// always points into the screen; a spacer of `HAIRLINE_GAP` separates the two.
pub fn edge_overlay<'a>(
    position: Position,
    content: Element<'a, Message>,
    indicator: Element<'a, Message>,
) -> Element<'a, Message> {
    let gap = container(iced::widget::Space::new()).height(HAIRLINE_GAP);
    match position {
        Position::Top => iced::widget::column![content, gap, indicator],
        Position::Bottom => iced::widget::column![indicator, gap, content],
    }
    .align_x(Alignment::Center)
    .into()
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

/// A state indicator expressed as the content's own container style, not a separately
/// sized sibling/overlay widget — the pattern real shipped iced panels use (cosmic-panel's
/// workspace applet renders its focused pill via `.class(Button::Suggested)` on the same
/// button, not a second element positioned to match it). `color: None` renders a plain,
/// unstyled container; `Some(color)` tints its background/border. Because the indicator
/// *is* the container wrapping `content`, it is trivially and always correctly sized —
/// this sidesteps the `Length::Fill`-vs-`Shrink`-compression subtlety a separate
/// hairline/overlay element must navigate (see `edge_overlay`'s doc comment for the bug
/// that pattern hit). Always call this (never conditionally), varying only `color`, so
/// the widget tree shape stays identical across renders regardless of state.
pub fn state_pill<'a>(
    config: &Config,
    content: Element<'a, Message>,
    color: Option<Color>,
) -> Element<'a, Message> {
    let radius = config.layout.focus_underline_thickness;
    container(content)
        .padding([2.0, config.layout.horizontal_widget_spacing as f32 * 0.5])
        .style(move |_| match color {
            Some(color) => container::Style {
                background: Some(Color { a: 0.22, ..color }.into()),
                border: Border {
                    color,
                    width: 1.0,
                    radius: radius.into(),
                },
                ..Default::default()
            },
            None => container::Style::default(),
        })
        .into()
}

/// odin's `Hairline`: a thin rounded bar marking an expandable group or focus/urgent
/// state. A fixed-height, content-width sibling (see `edge_overlay`), at the caller's
/// opacity.
pub fn hairline<'a>(config: &Config, color: Color, opacity: f32) -> Element<'a, Message> {
    let thickness = config.layout.focus_underline_thickness;
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
