use std::{iter::once, slice};

use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::Cursor,
    time::Duration,
    window,
};

use crate::widgets::{
    panel_frame::LayerFrameStyle,
    popup_connector::{ConnectorSide, ConnectorTriangle, FramedSurface},
    rectangle_morph::RectangleMorph,
    region_reporter::RegionRoles,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelGrowth {
    Downward,
    Upward,
}

#[derive(Debug, Clone, Copy)]
pub struct NotchPlacement {
    pub side: NotchSide,
    pub growth: PanelGrowth,
    pub row_top: f32,
    pub row_height: f32,
    pub notch_depth: f32,
}

impl NotchPlacement {
    fn row_center(self) -> f32 {
        self.row_top + self.row_height / 2.0
    }
}

pub struct NotchedPanel<'a, Message> {
    content: Element<'a, Message>,
    frame: LayerFrameStyle,
    corner_radius: f32,
    blurred: bool,
    placement: NotchPlacement,
    morph: Duration,
}

#[must_use]
pub fn notched_panel<'a, Message: 'a>(
    content: Element<'a, Message>,
    frame: LayerFrameStyle,
    blurred: bool,
    placement: NotchPlacement,
    morph: Duration,
) -> Element<'a, Message> {
    Element::new(NotchedPanel {
        content,
        corner_radius: frame.corner_radius().top_left,
        frame,
        blurred,
        placement,
        morph,
    })
}

#[derive(Default)]
struct PanelMorph {
    target: Rectangle,
    morph: RectangleMorph,
}

fn notch_side_radius(corner_radius: f32, panel_height: f32) -> f32 {
    let straight_half = panel_height / 2.0 - ConnectorTriangle::half_span();
    corner_radius.min(straight_half).max(0.0)
}

#[must_use]
pub fn notched_corners(side: NotchSide, corner_radius: f32, panel_height: f32) -> Radius {
    let notch_side = notch_side_radius(corner_radius, panel_height);
    match side {
        NotchSide::Left => Radius {
            top_left: notch_side,
            top_right: corner_radius,
            bottom_right: corner_radius,
            bottom_left: notch_side,
        },
        NotchSide::Right => Radius {
            top_left: corner_radius,
            top_right: notch_side,
            bottom_right: notch_side,
            bottom_left: corner_radius,
        },
    }
}

impl<Message> NotchedPanel<'_, Message> {
    fn panel_target(&self, content_size: Size) -> Rectangle {
        let panel_x = match self.placement.side {
            NotchSide::Left => self.placement.notch_depth,
            NotchSide::Right => 0.0,
        };
        let panel_y = match self.placement.growth {
            PanelGrowth::Downward => self.placement.row_top,
            PanelGrowth::Upward => {
                self.placement.row_top + self.placement.row_height - content_size.height
            }
        };
        Rectangle::new(Point::new(panel_x, panel_y), content_size)
    }

    fn folded_into_notch(&self, target: Rectangle) -> Rectangle {
        let notch_edge_x = match self.placement.side {
            NotchSide::Left => target.x,
            NotchSide::Right => target.x + target.width,
        };
        Rectangle::new(
            Point::new(
                notch_edge_x,
                self.placement.row_center() - self.corner_radius,
            ),
            Size::new(0.0, 2.0 * self.corner_radius),
        )
    }

    fn node_origin_from_notch_edge(&self, node_width: f32) -> Vector {
        match self.placement.side {
            NotchSide::Left => Vector::ZERO,
            NotchSide::Right => Vector::new(-node_width, 0.0),
        }
    }

    fn drawn_panel(&self, panel_morph: &PanelMorph, node_bounds: Rectangle) -> Rectangle {
        let notch_edge = Vector::new(node_bounds.x, node_bounds.y)
            - self.node_origin_from_notch_edge(node_bounds.width);
        panel_morph.morph.current() + notch_edge
    }

    fn notch_side_radius(&self, panel: Rectangle) -> f32 {
        notch_side_radius(self.corner_radius, panel.height)
    }

    fn panel_corners(&self, panel: Rectangle) -> Radius {
        notched_corners(self.placement.side, self.corner_radius, panel.height)
    }

    fn connector(&self, panel: Rectangle, origin: Vector) -> ConnectorTriangle {
        let notch_y = origin.y + self.placement.row_center();
        let apex_x = match self.placement.side {
            NotchSide::Left => panel.x - self.placement.notch_depth,
            NotchSide::Right => panel.x + panel.width + self.placement.notch_depth,
        };
        let side = match self.placement.side {
            NotchSide::Left => ConnectorSide::Left,
            NotchSide::Right => ConnectorSide::Right,
        };
        ConnectorTriangle::between(
            Rectangle::new(Point::new(apex_x, notch_y), Size::ZERO),
            apex_x,
            panel,
            side,
            self.notch_side_radius(panel),
        )
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for NotchedPanel<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<PanelMorph>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(PanelMorph::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let target = self.panel_target(content.size());
        let node_size = Size::new(
            target.width + self.placement.notch_depth,
            target.y + target.height,
        );
        let from_notch_edge = self.node_origin_from_notch_edge(node_size.width);
        let edge_target = target + from_notch_edge;
        let panel_morph: &mut PanelMorph = tree.state.downcast_mut();
        if panel_morph.target != edge_target {
            panel_morph.target = edge_target;
            panel_morph
                .morph
                .begin(self.folded_into_notch(target) + from_notch_edge);
        }
        panel_morph.morph.follow(edge_target);
        Node::with_children(node_size, vec![content.move_to(target.position())])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let panel_morph: &PanelMorph = tree.state.downcast_ref();
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        let panel = self.drawn_panel(panel_morph, layout.bounds());
        if panel.width <= 0.0 || panel.height <= 0.0 {
            return;
        }
        let frame_style = self.frame.container_style(theme);
        self.connector(panel, origin).draw_framed_surface(
            renderer,
            theme,
            FramedSurface {
                frame: self.frame,
                bounds: panel,
                corners: self.panel_corners(panel),
                shadow: frame_style.shadow,
            },
        );
        let Some(content_clip) = panel.intersection(viewport) else {
            return;
        };
        let content_style = renderer::Style {
            text_color: frame_style.text_color.unwrap_or(style.text_color),
        };
        let content_layout = layout.children().next().unwrap();
        renderer.with_layer(content_clip, |renderer| {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &content_style,
                content_layout,
                cursor,
                &content_clip,
            );
        });
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let panel_morph: &PanelMorph = tree.state.downcast_ref();
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        let panel = self.drawn_panel(panel_morph, layout.bounds());
        let rects: Vec<Rectangle> = once(panel)
            .chain(self.connector(panel, origin).blur_rows())
            .collect();
        RegionRoles {
            takes_input: true,
            blurred: self.blurred,
            corner_radius: self.panel_corners(panel),
        }
        .report(&rects, layout.bounds(), operation);
        let content_layout = layout.children().next().unwrap();
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            content_layout,
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let content_layout = layout.children().next().unwrap();
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            content_layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        let panel_morph: &mut PanelMorph = tree.state.downcast_mut();
        panel_morph.morph.tick(*now, self.morph);
        if !panel_morph.morph.is_settled() {
            shell.invalidate_layout();
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let content_layout = layout.children().next().unwrap();
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content_layout,
            cursor,
            viewport,
            renderer,
        )
    }
}
