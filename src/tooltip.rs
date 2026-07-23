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

use std::collections::BTreeMap;

use iced::{
    Border, Color, Element, Event, Length, Rectangle, Size, Task, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    widget::{Column, Sensor, container, row},
    window::Id,
};
use iced_layershell::reexport::{
    Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};

use crate::{
    bar_program::Message,
    data_sources::Row,
    layer_surface::{Position, Surface},
    user_config::Config,
    widgets::text::bar_text,
};

/// Gap between the bar edge and the tooltip (odin's `gap: 6`).
const GAP_PX: i32 = 6;
const PANEL_PADDING_PX: f32 = 10.0;
const ROW_SPACING_PX: f32 = 3.0;
const COLUMN_GAP_PX: f32 = 14.0;
/// A generous initial surface size. The panel lays itself out (shrink-to-fit)
/// inside it, iced reports the real size via a [`Sensor`], and the surface is
/// then shrunk to match — so no dimension is computed by hand. Only an upper
/// bound; the surface is transparent and click-through until it settles.
const PROVISIONAL_SIZE: (u32, u32) = (600, 800);

#[derive(Debug, Clone)]
pub enum TooltipMessage {
    /// Emitted by a hovered [`HoverArea`], which knows its anchor but not which
    /// surface it lives on. [`on_surface`] completes it into an [`Enter`] by
    /// stamping the surface id.
    ///
    /// [`Enter`]: TooltipMessage::Enter
    EnterAnchor {
        rows: Vec<Row>,
        anchor: Rectangle,
    },
    Enter {
        surface: Id,
        rows: Vec<Row>,
        anchor: Rectangle,
    },
    Exit {
        anchor: Rectangle,
    },
    Resized {
        id: Id,
        size: Size,
    },
}

struct Shown {
    id: Id,
    rows: Vec<Row>,
    /// The widget the tooltip belongs to, so a stray exit event from a sibling
    /// doesn't close it.
    anchor: Rectangle,
    /// Last surface size we requested, to ignore no-op resize reports.
    size: Option<(u32, u32)>,
}

/// The single active tooltip surface, if any. One at a time.
#[derive(Default)]
pub struct TooltipHost {
    shown: Option<Shown>,
}

impl TooltipHost {
    pub fn update(
        &mut self,
        message: TooltipMessage,
        surfaces: &BTreeMap<Id, Surface>,
        config: &Config,
    ) -> Task<Message> {
        match message {
            TooltipMessage::Enter {
                surface,
                rows,
                anchor,
            } => self.open(surface, rows, anchor, surfaces, config),
            // Completed into `Enter` by `on_surface` at the bar root before it
            // reaches here; a bare anchor means a hoverable escaped that wrapper.
            TooltipMessage::EnterAnchor { .. } => Task::none(),
            TooltipMessage::Exit { anchor } => self.exit(anchor),
            TooltipMessage::Resized { id, size } => self.resize(id, size),
        }
    }

    fn open(
        &mut self,
        surface: Id,
        rows: Vec<Row>,
        anchor: Rectangle,
        surfaces: &BTreeMap<Id, Surface>,
        config: &Config,
    ) -> Task<Message> {
        // A re-publish from the widget already shown: refresh the panel's rows
        // in place (the size sensor re-shrinks the surface if they re-measure)
        // instead of tearing down and reopening the surface.
        if let Some(shown) = &mut self.shown
            && shown.anchor == anchor
        {
            shown.rows = rows;
            return Task::none();
        }
        let Some(surface) = surfaces.get(&surface) else {
            return Task::none();
        };
        let settings = tooltip_settings(&surface.output, surface.position, anchor, config);
        let id = Id::unique();
        let close = self.close();
        self.shown = Some(Shown {
            id,
            rows,
            anchor,
            size: None,
        });
        Task::batch([close, Task::done(Message::NewLayerShell { settings, id })])
    }

    /// Close only when the widget being left is the one the tooltip belongs to:
    /// moving between adjacent readings can deliver the new enter before the
    /// old exit, and this keeps the newer hover.
    fn exit(&mut self, anchor: Rectangle) -> Task<Message> {
        if self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.anchor == anchor)
        {
            self.close()
        } else {
            Task::none()
        }
    }

    /// The tooltip surface's own layout pass measured the panel; shrink the
    /// surface to that exact size. Deduplicated so the settle doesn't loop.
    fn resize(&mut self, id: Id, size: Size) -> Task<Message> {
        let Some(shown) = &mut self.shown else {
            return Task::none();
        };
        let requested = (size.width.ceil() as u32, size.height.ceil() as u32);
        if shown.id != id || shown.size == Some(requested) {
            return Task::none();
        }
        shown.size = Some(requested);
        Task::done(Message::SizeChange {
            id,
            size: requested,
        })
    }

    /// Close the open tooltip surface, if any.
    pub fn close(&mut self) -> Task<Message> {
        match self.shown.take() {
            Some(shown) => Task::done(Message::RemoveWindow(shown.id)),
            None => Task::none(),
        }
    }

    pub fn shown_id(&self) -> Option<Id> {
        self.shown.as_ref().map(|shown| shown.id)
    }

    /// Drop the record of the open surface without asking to close it — for
    /// when the compositor has already closed it.
    pub fn forget(&mut self, id: Id) {
        if self.shown_id() == Some(id) {
            self.shown = None;
        }
    }

    /// Render the tooltip panel if `id` is the open tooltip surface. The panel
    /// is wrapped in a size sensor so iced's layout pass reports its true size
    /// back, which then drives the surface resize.
    pub fn view(&self, id: Id, config: &Config) -> Option<Element<'_, Message>> {
        let shown = self.shown.as_ref()?;
        (shown.id == id).then(|| {
            Sensor::new(panel(&shown.rows, config))
                .on_resize(move |size| Message::Tooltip(TooltipMessage::Resized { id, size }))
                .into()
        })
    }
}

/// Wrap a widget so hovering it opens a tooltip with `rows`, delivered as
/// `wrap(..)` so callers with their own message enums can embed the tooltip
/// messages. Inert (never publishes) when `rows` is empty, but still wraps, so
/// the widget tree keeps the same shape whether or not there is content. The
/// widget reports only its anchor on hover; the surface it lives on is stamped
/// later by [`on_surface`] at the bar root, so a hoverable widget need not know
/// which surface it is drawn into.
pub fn hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    rows: Vec<Row>,
    wrap: impl Fn(TooltipMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        rows,
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
pub fn on_surface(id: Id, content: Element<'_, Message>) -> Element<'_, Message> {
    content.map(move |message| match message {
        Message::Tooltip(TooltipMessage::EnterAnchor { rows, anchor }) => {
            Message::Tooltip(TooltipMessage::Enter {
                surface: id,
                rows,
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
    rows: Vec<Row>,
    wrap: Box<dyn Fn(TooltipMessage) -> M + 'a>,
}

#[derive(Default)]
struct HoverState {
    hovered: bool,
    /// The rows last published, so a data refresh while hovered re-publishes
    /// exactly once instead of every redraw.
    published: Vec<Row>,
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

        if self.rows.is_empty() {
            return;
        }
        let state: &mut HoverState = tree.state.downcast_mut();
        let anchor = layout.bounds();
        let hovered = cursor.is_over(anchor);
        if hovered != state.hovered {
            state.hovered = hovered;
            if hovered {
                state.published.clone_from(&self.rows);
            }
            shell.publish((self.wrap)(if hovered {
                TooltipMessage::EnterAnchor {
                    rows: self.rows.clone(),
                    anchor,
                }
            } else {
                TooltipMessage::Exit { anchor }
            }));
        } else if hovered && state.published != self.rows {
            // Fresh data while the tooltip is open: re-publish so the shown
            // panel tracks the reading instead of its enter-time snapshot.
            state.published.clone_from(&self.rows);
            shell.publish((self.wrap)(TooltipMessage::EnterAnchor {
                rows: self.rows.clone(),
                anchor,
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

fn panel<'a>(rows: &[Row], config: &Config) -> Element<'a, Message> {
    let foreground = config.palette.bar_foreground();
    let dim = Color {
        a: 0.7,
        ..foreground
    };
    // Two shrink-to-content columns (keys, values) side by side — like odin's
    // `TooltipRows` grid. Both size to their widest cell, so the panel is exactly
    // as wide as its content (no `Fill`, which would make it greedy and defeat
    // the `Sensor` measurement).
    let keys = Column::with_children(
        rows.iter()
            .map(|entry| bar_text(entry.key.clone()).color(dim).into()),
    )
    .spacing(ROW_SPACING_PX);
    let values = Column::with_children(
        rows.iter()
            .map(|entry| bar_text(entry.value.clone()).color(foreground).into()),
    )
    .spacing(ROW_SPACING_PX);
    let body = row![keys, values].spacing(COLUMN_GAP_PX);

    let background = Color {
        a: 1.0,
        ..config.palette.bar_background()
    };
    let radius = config.chrome_radius();
    container(body)
        .padding(PANEL_PADDING_PX)
        .style(move |_| container::Style {
            background: Some(background.into()),
            border: Border {
                color: Color {
                    a: 0.25,
                    ..foreground
                },
                width: 1.0,
                radius: radius.into(),
            },
            text_color: Some(foreground),
            ..Default::default()
        })
        .into()
}

fn tooltip_settings(
    output: &str,
    position: Position,
    anchor: Rectangle,
    config: &Config,
) -> NewLayerShellSettings {
    let bar = config.layout.outer_height_px as i32;
    // Left-align the panel under the hovered widget. Because the panel draws at
    // the surface's top-left, this edge stays put as the surface shrinks to fit,
    // so there is no reposition when the surface resizes.
    let left = (anchor.x as i32).max(0);
    let (anchor, margin) = match position {
        Position::Top => (Anchor::Top | Anchor::Left, (bar + GAP_PX, 0, 0, left)),
        Position::Bottom => (Anchor::Bottom | Anchor::Left, (0, 0, bar + GAP_PX, left)),
    };
    NewLayerShellSettings {
        size: Some(PROVISIONAL_SIZE),
        exclusive_zone: Some(0),
        anchor,
        margin: Some(margin),
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.to_string()),
        events_transparent: true,
        namespace: Some("odin-rs-tooltip".to_string()),
    }
}
