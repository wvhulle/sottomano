use std::{cell::Cell, rc::Rc, slice};

pub use corredo::popup_cascade::PanelGrowth;
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

use crate::{
    desktop_panels::{panel_frame::FrameStyle, region_reporter::RegionRoles},
    hover_cards::popup_connector::{ConnectorSide, ConnectorTriangle, FramedSurface},
    widgets::rectangle_morph::RectangleMorph,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedPanelTop(Rc<Cell<f32>>);

#[derive(Debug, Clone)]
pub enum NotchRowAnchor {
    FromPrimaryTop(f32),
    WithinPanel {
        panel_top: ResolvedPanelTop,
        row_offset: f32,
    },
}

impl NotchRowAnchor {
    fn row_top(&self) -> f32 {
        match self {
            NotchRowAnchor::FromPrimaryTop(row_top) => *row_top,
            NotchRowAnchor::WithinPanel {
                panel_top,
                row_offset,
            } => panel_top.0.get() + row_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScreenRoom {
    pub above_primary_top: f32,
    pub below_primary_top: f32,
}

impl ScreenRoom {
    pub(crate) fn limits_within(self, width: f32) -> Limits {
        Limits::new(
            Size::new(0.0, self.below_primary_top),
            Size::new(width, self.above_primary_top + self.below_primary_top),
        )
    }

    fn from_limits(limits: &Limits) -> Self {
        let below_primary_top = limits.min().height;
        Self {
            above_primary_top: limits.max().height - below_primary_top,
            below_primary_top,
        }
    }

    fn panel_top_on_screen(self, wanted_top: f32, panel_height: f32) -> f32 {
        let highest_top = -self.above_primary_top;
        let lowest_top = (self.below_primary_top - panel_height).max(highest_top);
        wanted_top.clamp(highest_top, lowest_top)
    }
}

#[derive(Debug, Clone)]
pub struct NotchPlacement {
    pub side: NotchSide,
    pub growth: PanelGrowth,
    pub row: NotchRowAnchor,
    pub panel_top: ResolvedPanelTop,
    pub row_height: f32,
    pub notch_depth: f32,
}

pub struct NotchedPanel<'a, Message> {
    content: Element<'a, Message>,
    frame: FrameStyle,
    corner_radius: f32,
    blurred: bool,
    placement: NotchPlacement,
    row_top: f32,
    morph: Duration,
}

#[must_use]
pub fn notched_panel<'a, Message: 'a>(
    content: Element<'a, Message>,
    frame: FrameStyle,
    blurred: bool,
    placement: NotchPlacement,
    morph: Duration,
) -> Element<'a, Message> {
    Element::new(NotchedPanel {
        content,
        corner_radius: frame.corner_radius().top_left,
        frame,
        blurred,
        row_top: placement.row.row_top(),
        placement,
        morph,
    })
}

#[derive(Default)]
struct PanelMorph {
    laid_out: Rectangle,
    laid_out_folded: Rectangle,
    morphing_toward: Rectangle,
    morph: RectangleMorph,
}

impl<Message> NotchedPanel<'_, Message> {
    fn row_center(&self) -> f32 {
        self.row_top + self.placement.row_height / 2.0
    }

    fn panel_target(&self, content_size: Size, screen_room: ScreenRoom) -> Rectangle {
        let panel_x = match self.placement.side {
            NotchSide::Left => self.placement.notch_depth,
            NotchSide::Right => 0.0,
        };
        let wanted_top = match self.placement.growth {
            PanelGrowth::Downward => self.row_top,
            PanelGrowth::Upward => self.row_top + self.placement.row_height - content_size.height,
        };
        let panel_y = screen_room.panel_top_on_screen(wanted_top, content_size.height);
        Rectangle::new(Point::new(panel_x, panel_y), content_size)
    }

    fn folded_into_notch(&self, target: Rectangle) -> Rectangle {
        let notch_edge_x = match self.placement.side {
            NotchSide::Left => target.x,
            NotchSide::Right => target.x + target.width,
        };
        Rectangle::new(
            Point::new(notch_edge_x, self.row_center() - self.corner_radius),
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

    fn panel_with_gap(&self, panel: Rectangle) -> Rectangle {
        let gap_left_x = match self.placement.side {
            NotchSide::Left => panel.x - self.placement.notch_depth,
            NotchSide::Right => panel.x,
        };
        Rectangle::new(
            Point::new(gap_left_x, panel.y),
            Size::new(panel.width + self.placement.notch_depth, panel.height),
        )
    }

    fn panel_corners(&self) -> Radius {
        Radius::from(self.corner_radius)
    }

    fn connector(&self, panel: Rectangle, origin: Vector) -> ConnectorTriangle {
        let notch_y = origin.y + self.row_center();
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
            0.0,
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
        let room = limits.max();
        let screen_room = ScreenRoom::from_limits(limits);
        self.row_top = self.placement.row.row_top();
        let content_limits = Limits::new(
            Size::ZERO,
            Size::new(
                (room.width - self.placement.notch_depth).max(0.0),
                room.height.max(self.placement.row_height),
            ),
        );
        let content =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &content_limits);
        let target = self.panel_target(content.size(), screen_room);
        self.placement.panel_top.0.set(target.y);
        let node_size = Size::new(
            target.width + self.placement.notch_depth,
            target.y + target.height,
        );
        let from_notch_edge = self.node_origin_from_notch_edge(node_size.width);
        let edge_target = target + from_notch_edge;
        let panel_morph: &mut PanelMorph = tree.state.downcast_mut();
        panel_morph.laid_out = edge_target;
        panel_morph.laid_out_folded = self.folded_into_notch(target) + from_notch_edge;
        panel_morph.morph.follow(panel_morph.morphing_toward);
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
                corners: self.panel_corners(),
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
        let drawn_surface = self
            .connector(panel, origin)
            .surface_rows(panel, self.panel_corners());
        RegionRoles {
            takes_input: false,
            blurred: self.blurred,
            corner_radius: Radius::default(),
        }
        .report(&drawn_surface, layout.bounds(), operation);
        RegionRoles {
            takes_input: true,
            blurred: false,
            corner_radius: Radius::default(),
        }
        .report(&[self.panel_with_gap(panel)], layout.bounds(), operation);
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
        if panel_morph.laid_out != panel_morph.morphing_toward {
            panel_morph.morphing_toward = panel_morph.laid_out;
            panel_morph.morph.begin(panel_morph.laid_out_folded);
        }
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

#[cfg(test)]
mod screen_room_tests {
    use super::ScreenRoom;

    const ROOM_OVER_BOTTOM_PANEL: ScreenRoom = ScreenRoom {
        above_primary_top: 300.0,
        below_primary_top: 100.0,
    };

    #[test]
    fn a_panel_that_fits_stays_at_its_row() {
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(-40.0, 100.0) + 40.0).abs() < f32::EPSILON
        );
    }

    #[test]
    fn a_panel_past_either_screen_edge_slides_just_back_on_screen() {
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(-350.0, 350.0) + 300.0).abs()
                < f32::EPSILON
        );
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(40.0, 350.0) + 250.0).abs() < f32::EPSILON
        );
    }

    #[test]
    fn a_panel_taller_than_the_screen_starts_at_the_screen_top() {
        assert!(
            (ROOM_OVER_BOTTOM_PANEL.panel_top_on_screen(40.0, 400.0) + 300.0).abs() < f32::EPSILON
        );
    }
}
