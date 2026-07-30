//! Hover tooltips, rendered on their own Wayland surface.
//!
//! The bar surface is exactly bar-height, so a tooltip below/above it cannot be
//! drawn in-surface. Instead — like odin's popup-backed `HoverTooltip` — each
//! tooltip is a separate layer-shell surface, positioned under the hovered
//! widget and made click-through (`events_transparent`) so it never steals
//! input. Content is the widget's own rows, rendered as a two-column
//! key/value panel; no subprocess is involved.
//!
//! Positioning is encapsulated in the widget: [`hoverable`] wraps a widget in a
//! [`HoverArea`], which reads its own `layout.bounds()` and reports them as the
//! tooltip anchor on hover — no global cursor. The panel then sizes itself: a
//! `Sensor` reports iced's measured size, which shrinks the surface to fit.

use std::{collections::HashMap, time::SystemTime};

use iced::{
    Border, Color, Element, Event, Font, Length, Padding, Rectangle, Size, Task, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    font::{Style as FontStyle, Weight},
    widget::{Column, Sensor, container, row},
    window::Id,
};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    app::Message,
    config::resolved::ResolvedConfig,
    data::source::{RefreshInfo, Tooltip, TooltipBody, level_color, niri::OutputGeometry},
    wayland_layers::surface::{Margin, Position, Surface, SurfaceSize},
    widgets::text::{TextStyle, bar_text},
};

/// Which hoverable region a tooltip belongs to — the one identity that rides
/// the hover message. The hovered widget publishes only this; the panel content
/// is re-derived live from it each frame by the owning group (see
/// [`Bar::tooltip_for`]), so an open tooltip always tracks its reading and
/// nothing is computed while nothing is hovered. The two multi-instance
/// families carry their instance's real identifier as data (a display serial, a
/// tunnel interface name), stable across list reordering.
///
/// [`Bar::tooltip_for`]: crate::app::Bar
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoverTarget {
    Cpu,
    Memory,
    Temperature,
    Disk,
    Battery,
    PowerDraw,
    Floating,
    FloatOffset,
    JournalSystem,
    JournalUser,
    JournalKernel,
    Bluetooth,
    Ethernet,
    Wifi,
    Mobile,
    AudioSink,
    AudioSource,
    PowerProfile,
    /// One display's hardware backlight, by its serial.
    Backlight(String),
    GammaDim,
    ColorTemperature,
    ClightAuto,
    DarkMode,
    IdleLockUser,
    IdleScreenOff,
    IdleStandby,
    IdlePowerDown,
    InternetDown,
    InternetUp,
    Tailscale,
    /// One `WireGuard` tunnel, by its interface name.
    Wireguard(String),
    WeatherTemp,
    WeatherRain,
    PollenBirch,
    PollenGrass,
    Location,
    Media,
    AiOpenRouter,
    AiClaudeOauth,
    AiClaudeCcusage,
    AiCopilotMetered,
    AiCopilotFree,
    AiCopilotCli,
    /// A tray item, carrying its own static tooltip content (title +
    /// description). Unlike the gauges, a tray tooltip does not change per
    /// frame, so it rides in the target itself rather than being re-derived
    /// from a reading.
    Tray(Box<Tooltip>),
}

/// Gap between the bar edge and the tooltip (odin's `gap: 6`).
const GAP_PX: i32 = 6;
const PANEL_PADDING: Padding = Padding::new(10.0);
const ROW_SPACING_PX: f32 = 3.0;
const COLUMN_GAP_PX: f32 = 14.0;
/// Gap between the tooltip body grid and its refresh footer.
const FOOTER_GAP_PX: f32 = 6.0;
const FOOTER_SCALE: f32 = 0.85;
const FOOTER_PADDING: Padding = Padding {
    top: 2.0,
    right: 6.0,
    bottom: 2.0,
    left: 6.0,
};

#[derive(Debug, Clone)]
pub enum TooltipMessage {
    /// Emitted by a hovered [`HoverArea`], which knows its anchor but not which
    /// surface it lives on. [`on_surface`] completes it into an [`Enter`] by
    /// stamping the surface id.
    ///
    /// [`Enter`]: TooltipMessage::Enter
    EnterAnchor {
        target: HoverTarget,
        anchor: Rectangle,
    },
    Enter {
        surface: Id,
        target: HoverTarget,
        anchor: Rectangle,
    },
    Exit {
        target: HoverTarget,
    },
    Resized {
        surface: Id,
        size: Size,
    },
}

struct Shown {
    surface: Id,
    /// The widget this tooltip belongs to: drives content derivation and guards
    /// against a stray exit from a sibling closing it.
    target: HoverTarget,
    /// Last surface size we requested, to ignore no-op resize reports.
    size: Option<SurfaceSize>,
    /// The hovered widget's bounds; `anchor.x` positions the panel
    /// horizontally.
    anchor: Rectangle,
    /// The output's logical width, for right-edge placement math. `None` when
    /// the output geometry is unknown — placement then falls back to
    /// `anchor.x`.
    output_width: Option<i32>,
    /// The output the parent bar lives on, so a popup anchored to the same
    /// hovered widget (the tray context menu) can bind to the same output.
    output: String,
    /// Which screen edge the parent bar hugs, to rebuild the vertical margin
    /// when the surface settles.
    position: Position,
}

/// Placement of a popup anchored to a hovered widget, reused by the tray
/// context menu so it lands under the same icon the tooltip would.
#[derive(Debug, Clone)]
pub struct MenuAnchor {
    pub anchor: Rectangle,
    pub output: String,
    pub output_width: Option<i32>,
    pub position: Position,
}

/// The single active tooltip surface, if any. One at a time.
#[derive(Default)]
pub struct TooltipHost {
    shown: Option<Shown>,
}

impl TooltipHost {
    pub fn update(&mut self, message: TooltipMessage, config: &ResolvedConfig) -> Task<Message> {
        match message {
            TooltipMessage::Enter { .. } | TooltipMessage::EnterAnchor { .. } => Task::none(),
            TooltipMessage::Exit { target } => self.exit(&target),
            TooltipMessage::Resized { surface, size } => self.resize(surface, size, config),
        }
    }

    pub fn open(
        &mut self,
        parent: &Surface,
        target: HoverTarget,
        anchor: Rectangle,
        outputs: &HashMap<String, OutputGeometry>,
        config: &ResolvedConfig,
        content: &Tooltip,
    ) -> Task<Message> {
        // Already showing this target (a stray re-enter): keep the open surface
        // rather than tearing it down — content refreshes at view time.
        if self.shown.as_ref().is_some_and(|s| s.target == target) {
            return Task::none();
        }
        let output_width = outputs.get(&parent.output).map(|geometry| geometry.width);
        let measured = measure_panel(content, config);
        let clamped = match output_width {
            Some(width) => Size::new(measured.width.min(width.max(0) as f32), measured.height),
            None => measured,
        };
        let size = surface_size(clamped);
        let settings = tooltip_settings(
            &parent.output,
            parent.position,
            anchor,
            output_width,
            size,
            config,
        );
        let surface = Id::unique();
        let close = self.close();
        self.shown = Some(Shown {
            surface,
            target,
            size: Some(size),
            anchor,
            output_width,
            output: parent.output.clone(),
            position: parent.position,
        });
        Task::batch([
            close,
            Task::done(Message::NewLayerShell {
                settings,
                id: surface,
            }),
        ])
    }

    /// Close only when the widget being left is the one the tooltip belongs to:
    /// moving between adjacent readings can deliver the new enter before the
    /// old exit, and this keeps the newer hover.
    fn exit(&mut self, target: &HoverTarget) -> Task<Message> {
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| &shown.target == target)
        {
            self.close()
        } else {
            Task::none()
        }
    }

    /// The tooltip surface's own layout pass measured the panel; shrink the
    /// surface to that exact size, and re-place its left margin now that the
    /// true content width is known — so a near-right-edge panel sits flush to
    /// the screen edge (drawn to the left of the cursor) at full width instead
    /// of being clamped narrow. Deduplicated so the settle doesn't loop.
    fn resize(&mut self, surface: Id, size: Size, config: &ResolvedConfig) -> Task<Message> {
        let Some(shown) = &mut self.shown else {
            return Task::none();
        };
        let requested = surface_size(size);
        if shown.surface != surface || shown.size == Some(requested) {
            return Task::none();
        }
        shown.size = Some(requested);
        let bar = config.layout.outer_height_px as i32;
        let left = panel_left(shown.anchor.x, shown.output_width, requested.width);
        let margin = tooltip_margin(shown.position, left, bar);
        Task::batch([
            Task::done(Message::MarginChange {
                id: surface,
                margin: margin.edges(),
            }),
            Task::done(Message::SizeChange {
                id: surface,
                size: requested.dimensions(),
            }),
        ])
    }

    /// Close the open tooltip surface, if any.
    pub fn close(&mut self) -> Task<Message> {
        match self.shown.take() {
            Some(shown) => Task::done(Message::RemoveWindow(shown.surface)),
            None => Task::none(),
        }
    }

    pub fn shown_surface(&self) -> Option<Id> {
        self.shown.as_ref().map(|shown| shown.surface)
    }

    /// Drop the record of the open surface without asking to close it — for
    /// when the compositor has already closed it.
    pub fn forget(&mut self, surface: Id) {
        if self.shown_surface() == Some(surface) {
            self.shown = None;
        }
    }

    /// The hover target drawn on `surface`, if that is the open tooltip. The
    /// caller derives live content from it and passes it to [`panel_surface`].
    pub fn target_for(&self, surface: Id) -> Option<&HoverTarget> {
        self.shown
            .as_ref()
            .filter(|shown| shown.surface == surface)
            .map(|shown| &shown.target)
    }

    /// The open tooltip's hover target, if any — for gating the relative-time
    /// tick on whether a [`TooltipBody::Log`] is shown.
    pub fn target(&self) -> Option<&HoverTarget> {
        self.shown.as_ref().map(|shown| &shown.target)
    }

    /// Placement for a popup anchored to the currently-hovered tray item — its
    /// icon bounds, output, and edge — reused by the tray context menu. `None`
    /// unless a tray tooltip is open (right-clicking an icon means it is
    /// hovered, so its tooltip is showing and carries the anchor).
    pub fn menu_anchor(&self) -> Option<MenuAnchor> {
        let shown = self.shown.as_ref()?;
        matches!(shown.target, HoverTarget::Tray(_)).then(|| MenuAnchor {
            anchor: shown.anchor,
            output: shown.output.clone(),
            output_width: shown.output_width,
            position: shown.position,
        })
    }
}

/// Render `tooltip` as the panel for surface `id`, wrapped in a size sensor so
/// iced's layout pass reports its true size back, which drives the surface
/// resize. Content is derived by the caller from the live reading each render.
pub fn panel_surface<'a>(
    surface: Id,
    tooltip: &Tooltip,
    config: &ResolvedConfig,
) -> Element<'a, Message> {
    Sensor::new(panel(tooltip, config))
        .on_resize(move |size| Message::Tooltip(TooltipMessage::Resized { surface, size }))
        .into()
}

/// Wrap a widget so hovering it opens a tooltip for `target`, with the tooltip
/// messages delivered as `wrap(..)` so callers with their own message enums can
/// embed them. The widget reports only its `target` and anchor on hover — the
/// panel content is derived from the live reading at render time (the owning
/// group resolves `target` back to a reading, see [`Bar::tooltip_for`]) — and
/// the surface it lives on is stamped later by [`on_surface`] at the bar root,
/// so a hoverable widget need not know which surface it is drawn into.
///
/// [`Bar::tooltip_for`]: crate::app::Bar
pub fn hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    wrap: impl Fn(TooltipMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        wrap: Box::new(wrap),
    })
}

/// Stamp the surface `id` onto every tooltip [`EnterAnchor`] bubbling out of
/// `content`, turning it into an [`Enter`] the host can place. Applied once, at
/// the bar root, by the code that owns the surface→id table — the only layer
/// that knows a widget's surface (iced strips the window id from widget-emitted
/// messages, so a `HoverArea` cannot supply it itself).
///
/// [`EnterAnchor`]: TooltipMessage::EnterAnchor
/// [`Enter`]: TooltipMessage::Enter
pub fn on_surface(surface: Id, content: Element<'_, Message>) -> Element<'_, Message> {
    content.map(move |message| match message {
        Message::Tooltip(TooltipMessage::EnterAnchor { target, anchor }) => {
            Message::Tooltip(TooltipMessage::Enter {
                surface,
                target,
                anchor,
            })
        }
        other => other,
    })
}

/// Reports its content's absolute bounds as the tooltip anchor when hovered.
/// Everything else delegates straight to the content, so it is invisible to
/// layout and drawing.
struct HoverArea<'a, M> {
    content: Element<'a, M>,
    target: HoverTarget,
    wrap: Box<dyn Fn(TooltipMessage) -> M + 'a>,
}

#[derive(Default)]
struct HoverState {
    hovered: bool,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for HoverArea<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HoverState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let state: &mut HoverState = tree.state.downcast_mut();
        let anchor = layout.bounds();
        let hovered = cursor.is_over(anchor);
        if hovered != state.hovered {
            state.hovered = hovered;
            shell.publish((self.wrap)(if hovered {
                TooltipMessage::EnterAnchor {
                    target: self.target.clone(),
                    anchor,
                }
            } else {
                TooltipMessage::Exit {
                    target: self.target.clone(),
                }
            }));
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// The tooltip body as a laid-out element. Table/Log render as two
/// shrink-to-content columns side by side — odin's `TooltipRows` grid; both
/// size to their widest cell, so the panel is exactly as wide as its content
/// (no `Fill`, which would make it greedy and defeat the `Sensor` measurement).
/// A Table shows dimmed key | value; a Log shows dimmed relative time |
/// level-tinted message; Text renders as a bold title stacked over a dimmed
/// body (a tray item's own tooltip).
fn tooltip_grid<'a>(
    body: &TooltipBody,
    config: &ResolvedConfig,
    foreground: Color,
    dim: Color,
    now: SystemTime,
) -> Element<'a, Message> {
    match body {
        TooltipBody::Table(rows) => row![
            Column::with_children(
                rows.iter()
                    .map(|entry| bar_text(&config.font, entry.key.clone()).color(dim).into()),
            )
            .spacing(ROW_SPACING_PX),
            Column::with_children(rows.iter().map(|entry| {
                bar_text(&config.font, entry.value.clone())
                    .color(foreground)
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
        ]
        .spacing(COLUMN_GAP_PX)
        .into(),
        TooltipBody::Log(entries) => row![
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&config.font, relative(entry.at, now))
                    .color(dim)
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
            Column::with_children(entries.iter().map(|entry| {
                bar_text(&config.font, entry.message.clone())
                    .color(level_color(config, entry.level))
                    .into()
            }))
            .spacing(ROW_SPACING_PX),
        ]
        .spacing(COLUMN_GAP_PX)
        .into(),
        TooltipBody::Text { title, body } => {
            let bold = Font {
                weight: Weight::Bold,
                ..config.font.iced_font
            };
            let mut column = Column::new().spacing(ROW_SPACING_PX);
            if !title.is_empty() {
                column = column.push(
                    bar_text(&config.font, title.clone())
                        .font(bold)
                        .color(foreground),
                );
            }
            if !body.is_empty() {
                column = column.push(bar_text(&config.font, body.clone()).color(dim));
            }
            column.into()
        }
    }
}

fn footer_text(info: RefreshInfo, now: SystemTime) -> String {
    format!(
        "updated {} · every {}",
        relative(info.last, now),
        humantime::format_duration(info.every),
    )
}

fn surface_size(size: Size) -> SurfaceSize {
    SurfaceSize {
        width: size.width.ceil() as u32,
        height: size.height.ceil() as u32,
    }
}

fn column<'a>(lines: impl Iterator<Item = &'a str>, style: TextStyle) -> Size {
    let lines: Vec<&str> = lines.collect();
    let measured = style.measure(&lines.join("\n"));
    Size::new(
        measured.width,
        measured.height + lines.len().saturating_sub(1) as f32 * ROW_SPACING_PX,
    )
}

fn beside(left: Size, right: Size) -> Size {
    Size::new(
        left.width + COLUMN_GAP_PX + right.width,
        left.height.max(right.height),
    )
}

fn stack(top: Size, bottom: Size, gap: f32) -> Size {
    let gap = if top.height > 0.0 && bottom.height > 0.0 {
        gap
    } else {
        0.0
    };
    Size::new(
        top.width.max(bottom.width),
        top.height + gap + bottom.height,
    )
}

fn measure_panel(tooltip: &Tooltip, config: &ResolvedConfig) -> Size {
    let base = TextStyle::of(&config.font);
    let now = SystemTime::now();

    let grid = match &tooltip.body {
        TooltipBody::Table(rows) => beside(
            column(rows.iter().map(|entry| entry.key.as_str()), base),
            column(rows.iter().map(|entry| entry.value.as_str()), base),
        ),
        TooltipBody::Log(entries) => {
            let times: Vec<String> = entries
                .iter()
                .map(|entry| relative(entry.at, now))
                .collect();
            beside(
                column(times.iter().map(String::as_str), base),
                column(entries.iter().map(|entry| entry.message.as_str()), base),
            )
        }
        TooltipBody::Text { title, body } => stack(
            base.bold().measure(title),
            base.measure(body),
            ROW_SPACING_PX,
        ),
    };

    let body = match tooltip.footer {
        Some(info) => {
            let footer = base
                .italic()
                .scaled(FOOTER_SCALE)
                .measure(&footer_text(info, now))
                .expand(FOOTER_PADDING);
            stack(grid, footer, FOOTER_GAP_PX)
        }
        None => grid,
    };

    body.expand(PANEL_PADDING)
}

fn panel<'a>(tooltip: &Tooltip, config: &ResolvedConfig) -> Element<'a, Message> {
    let foreground = config.palette.bar_foreground;
    let dim = Color {
        a: 0.7,
        ..config.palette.bar_foreground
    };
    let now = SystemTime::now();
    let grid = tooltip_grid(&tooltip.body, config, foreground, dim, now);

    let italic = Font {
        style: FontStyle::Italic,
        ..config.font.iced_font
    };
    let body = match tooltip.footer {
        Some(info) => {
            let footer = container(
                bar_text(&config.font, footer_text(info, now))
                    .size(config.font.text_size_px * FOOTER_SCALE)
                    .font(italic)
                    .color(dim),
            )
            .padding(FOOTER_PADDING);
            Column::with_children([grid, footer.into()])
                .spacing(FOOTER_GAP_PX)
                .into()
        }
        None => grid,
    };

    let background = Color {
        a: 1.0,
        ..config.palette.bar_background
    };
    let radius = f32::from(config.layout.corner_radius_px);
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(PANEL_PADDING)
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: radius.into(),
            },
            text_color: Some(foreground),
            ..Default::default()
        })
        .into()
}

/// `at` as a coarse relative time, ported from odin's `RelativeTime.format`:
/// `just now` under 5 s, then `N s|min|h|d ago`. `now` is passed so the caller
/// samples the clock once per render; a future `at` degrades to `just now`.
pub(crate) fn relative(at: SystemTime, now: SystemTime) -> String {
    let secs = now.duration_since(at).map_or(0, |d| d.as_secs());
    match secs {
        0..=4 => "just now".to_string(),
        5..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} d ago", secs / 86_400),
    }
}

/// The panel's left margin for a surface `panel_width` px wide. Left-aligned
/// under the hovered widget (`anchor_x`), but when the panel would overflow the
/// right screen edge it is shifted left to sit flush against it — so a
/// right-edge tooltip is drawn to the left of the cursor at full width instead
/// of being clamped narrow. `output_width` is `None` when the geometry is
/// unknown, in which case the widget's `anchor_x` is used unchanged (today's
/// behaviour, no regression). Never negative.
pub(crate) fn panel_left(anchor_x: f32, output_width: Option<i32>, panel_width: u32) -> i32 {
    let left = anchor_x as i32;
    let left = match output_width {
        Some(width) => left.min(width - panel_width as i32),
        None => left,
    };
    left.max(0)
}

/// The four-sided surface margin `(top, right, bottom, left)` placing the panel
/// `left` px from the output's left edge and `bar + GAP_PX` off the bar edge.
pub(crate) fn tooltip_margin(position: Position, left: i32, bar: i32) -> Margin {
    match position {
        Position::Top => Margin {
            top: bar + GAP_PX,
            right: 0,
            bottom: 0,
            left,
        },
        Position::Bottom => Margin {
            top: 0,
            right: 0,
            bottom: bar + GAP_PX,
            left,
        },
    }
}

fn tooltip_settings(
    output: &str,
    position: Position,
    anchor: Rectangle,
    output_width: Option<i32>,
    size: SurfaceSize,
    config: &ResolvedConfig,
) -> NewLayerShellSettings {
    let bar = config.layout.outer_height_px as i32;
    let left = panel_left(anchor.x, output_width, size.width);
    let anchor = match position {
        Position::Top => Anchor::Top | Anchor::Left,
        Position::Bottom => Anchor::Bottom | Anchor::Left,
    };
    NewLayerShellSettings {
        size: Some(size.dimensions()),
        exclusive_zone: Some(0),
        anchor,
        margin: Some(tooltip_margin(position, left, bar).edges()),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        events_transparent: true,
        namespace: Some("vetrata-tooltip".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ago(secs: u64) -> String {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        relative(now - Duration::from_secs(secs), now)
    }

    #[test]
    fn relative_time_buckets() {
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(4), "just now");
        assert_eq!(ago(5), "5 s ago");
        assert_eq!(ago(59), "59 s ago");
        assert_eq!(ago(60), "1 min ago");
        assert_eq!(ago(3599), "59 min ago");
        assert_eq!(ago(3600), "1 h ago");
        assert_eq!(ago(86_399), "23 h ago");
        assert_eq!(ago(86_400), "1 d ago");
    }

    #[test]
    fn future_timestamp_is_just_now() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert_eq!(relative(now + Duration::from_secs(60), now), "just now");
    }
}
