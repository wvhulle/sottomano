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
use sottomano_config::screen_edges::Position;

use crate::widgets::{
    panel_frame::LayerFrameStyle,
    popup_connector::{ConnectorTriangle, FramedSurface},
    rectangle_morph::RectangleMorph,
    region_reporter::RegionRoles,
};

#[derive(Debug, Clone, Copy)]
pub struct NotchPlacement {
    pub top_offset: f32,
    pub notch_center: f32,
    pub notch_depth: f32,
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
pub fn notched_corners(corner_radius: f32, panel_height: f32) -> Radius {
    let notch_side = notch_side_radius(corner_radius, panel_height);
    Radius {
        top_left: notch_side,
        top_right: corner_radius,
        bottom_right: corner_radius,
        bottom_left: notch_side,
    }
}

impl<Message> NotchedPanel<'_, Message> {
    fn panel_target(&self, content_size: Size) -> Rectangle {
        Rectangle::new(
            Point::new(self.placement.notch_depth, self.placement.top_offset),
            content_size,
        )
    }

    fn folded_into_notch(&self, target: Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(
                target.x,
                target.y + self.placement.notch_center - self.corner_radius,
            ),
            Size::new(0.0, 2.0 * self.corner_radius),
        )
    }

    fn notch_side_radius(&self, panel: Rectangle) -> f32 {
        notch_side_radius(self.corner_radius, panel.height)
    }

    fn panel_corners(&self, panel: Rectangle) -> Radius {
        notched_corners(self.corner_radius, panel.height)
    }

    fn connector(&self, panel: Rectangle, origin: Vector) -> ConnectorTriangle {
        let notch_y = origin.y + self.placement.top_offset + self.placement.notch_center;
        let apex_x = panel.x - self.placement.notch_depth;
        ConnectorTriangle::between(
            Rectangle::new(Point::new(apex_x, notch_y), Size::ZERO),
            apex_x,
            panel,
            Position::Left,
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
        let panel_morph: &mut PanelMorph = tree.state.downcast_mut();
        if panel_morph.target != target {
            panel_morph.target = target;
            panel_morph.morph.begin(self.folded_into_notch(target));
        }
        panel_morph.morph.follow(target);
        Node::with_children(
            Size::new(target.x + target.width, target.y + target.height),
            vec![content.move_to(target.position())],
        )
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
        let panel = panel_morph.morph.current() + origin;
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
        let panel = panel_morph.morph.current() + origin;
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
