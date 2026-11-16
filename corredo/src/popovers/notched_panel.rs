use std::iter::once;

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
    widget::container,
    window,
};

use crate::{
    orientation::Edge,
    popovers::{
        CascadeDirection, CascadeOrientation, ConnectedSurface, ConnectorTriangle, PopupGrowth,
        RectangleMorph, SubmenuConnector, cascade_row::drawn_extent,
    },
    theming::FrameStyle,
    transitions::MotionState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotchSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NotchRowAnchor {
    FromPrimaryTop(f32),
    WithinParent { row_offset: f32 },
}

impl NotchRowAnchor {
    fn row_top(self, parent_panel_top: f32) -> f32 {
        match self {
            NotchRowAnchor::FromPrimaryTop(row_top) => row_top,
            NotchRowAnchor::WithinParent { row_offset } => parent_panel_top + row_offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelNesting {
    Outermost,
    BesideParent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScreenRoom {
    pub above_primary_top: f32,
    pub below_primary_top: f32,
}

impl From<&Limits> for ScreenRoom {
    fn from(limits: &Limits) -> Self {
        let below_primary_top = limits.min().height;
        Self {
            above_primary_top: limits.max().height - below_primary_top,
            below_primary_top,
        }
    }
}

impl ScreenRoom {
    pub(crate) fn limits_within(self, width: f32) -> Limits {
        Limits::new(
            Size::new(0.0, self.below_primary_top),
            Size::new(width, self.above_primary_top + self.below_primary_top),
        )
    }

    fn panel_top_on_screen(self, wanted_top: f32, panel_height: f32) -> f32 {
        let highest_top = -self.above_primary_top;
        let lowest_top = (self.below_primary_top - panel_height).max(highest_top);
        wanted_top.clamp(highest_top, lowest_top)
    }
}

const DEFAULT_MORPH: Duration = Duration::from_millis(160);

#[derive(Debug, Clone)]
struct NotchPlacement {
    side: NotchSide,
    growth: PopupGrowth,
    row: NotchRowAnchor,
    row_height: f32,
    gap_to_parent: f32,
    submenu_connector: SubmenuConnector,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NotchedPanelRegions {
    pub surface_rows: Vec<Rectangle>,
    pub surface_corners: Radius,
    pub accepting_input: Rectangle,
    pub bounds: Rectangle,
    pub nesting: PanelNesting,
}

pub struct NotchedPanel<'a, Message> {
    content: Element<'a, Message>,
    frame: FrameStyle,
    corner_radius: f32,
    placement: NotchPlacement,
    row_top: f32,
    morph: Duration,
    beside: Vec<NotchedPanel<'a, Message>>,
    parent_panel_top: f32,
    nesting: PanelNesting,
}

#[must_use]
pub fn notched_panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    orientation: CascadeOrientation,
    row: NotchRowAnchor,
) -> NotchedPanel<'a, Message> {
    let frame = FrameStyle::default();
    NotchedPanel {
        content: content.into(),
        corner_radius: frame.corner_radius().top_left,
        frame,
        row_top: row.row_top(0.0),
        placement: NotchPlacement {
            side: match orientation.direction {
                CascadeDirection::Rightward => NotchSide::Left,
                CascadeDirection::Leftward => NotchSide::Right,
            },
            growth: orientation.growth,
            row,
            row_height: 0.0,
            gap_to_parent: 0.0,
            submenu_connector: SubmenuConnector::default(),
        },
        morph: DEFAULT_MORPH,
        beside: Vec::new(),
        parent_panel_top: 0.0,
        nesting: PanelNesting::Outermost,
    }
}

impl<'a, Message: 'a> NotchedPanel<'a, Message> {
    #[must_use]
    pub fn frame(self, frame: FrameStyle) -> Self {
        Self {
            corner_radius: frame.corner_radius().top_left,
            frame,
            ..self
        }
    }

    #[must_use]
    pub fn row_height(self, row_height: f32) -> Self {
        Self {
            placement: NotchPlacement {
                row_height,
                ..self.placement
            },
            ..self
        }
    }

    #[must_use]
    pub fn gap_to_parent(self, gap_to_parent: f32) -> Self {
        Self {
            placement: NotchPlacement {
                gap_to_parent,
                ..self.placement
            },
            ..self
        }
    }

    #[must_use]
    pub fn submenu_connector(self, submenu_connector: SubmenuConnector) -> Self {
        Self {
            placement: NotchPlacement {
                submenu_connector,
                ..self.placement
            },
            ..self
        }
    }

    #[must_use]
    pub fn morph(self, morph: Duration) -> Self {
        Self { morph, ..self }
    }

    #[must_use]
    pub fn beside(self, beside: impl IntoIterator<Item = NotchedPanel<'a, Message>>) -> Self {
        Self {
            beside: beside
                .into_iter()
                .map(|panel| NotchedPanel {
                    nesting: PanelNesting::BesideParent,
                    ..panel
                })
                .collect(),
            ..self
        }
    }
}

impl<'a, Message: 'a> From<NotchedPanel<'a, Message>> for Element<'a, Message> {
    fn from(notched_panel: NotchedPanel<'a, Message>) -> Self {
        Element::new(notched_panel)
    }
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
            NotchSide::Left => self.placement.gap_to_parent,
            NotchSide::Right => 0.0,
        };
        let wanted_top = match self.placement.growth {
            PopupGrowth::Downward => self.row_top,
            PopupGrowth::Upward => self.row_top + self.placement.row_height - content_size.height,
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
        panel_morph.morph.current + notch_edge
    }

    fn panel_with_gap(&self, panel: Rectangle) -> Rectangle {
        let gap_left_x = match self.placement.side {
            NotchSide::Left => panel.x - self.placement.gap_to_parent,
            NotchSide::Right => panel.x,
        };
        Rectangle::new(
            Point::new(gap_left_x, panel.y),
            Size::new(panel.width + self.placement.gap_to_parent, panel.height),
        )
    }

    fn panel_corners(&self) -> Radius {
        Radius::from(self.corner_radius)
    }

    fn connector(&self, panel: Rectangle, origin: Vector) -> ConnectorTriangle {
        let notch_y = origin.y + self.row_center();
        let apex_x = match self.placement.side {
            NotchSide::Left => panel.x - self.placement.gap_to_parent,
            NotchSide::Right => panel.x + panel.width + self.placement.gap_to_parent,
        };
        let side = match self.placement.side {
            NotchSide::Left => Edge::Left,
            NotchSide::Right => Edge::Right,
        };
        ConnectorTriangle::between(
            Rectangle::new(Point::new(apex_x, notch_y), Size::ZERO),
            apex_x,
            panel,
            side,
            0.0,
        )
    }

    fn drawn_surface_rows(&self, panel: Rectangle, origin: Vector) -> Vec<Rectangle> {
        match self.placement.submenu_connector {
            SubmenuConnector::Shown => self
                .connector(panel, origin)
                .surface_rows(panel, self.panel_corners()),
            SubmenuConnector::Hidden => vec![panel],
        }
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
        once(Tree::new(&self.content))
            .chain(
                self.beside.iter().map(|panel| {
                    Tree::new(panel as &dyn Widget<Message, iced::Theme, iced::Renderer>)
                }),
            )
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let widgets: Vec<&dyn Widget<Message, iced::Theme, iced::Renderer>> =
            once(self.content.as_widget())
                .chain(
                    self.beside
                        .iter()
                        .map(|panel| panel as &dyn Widget<Message, iced::Theme, iced::Renderer>),
                )
                .collect();
        tree.diff_children(&widgets);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let room = limits.max();
        let screen_room = ScreenRoom::from(limits);
        self.row_top = self.placement.row.row_top(self.parent_panel_top);
        let gap_to_parent = self.placement.gap_to_parent;
        let content_limits = Limits::new(
            Size::ZERO,
            Size::new(
                (room.width - gap_to_parent).max(0.0),
                room.height.max(self.placement.row_height),
            ),
        );
        let (content_tree, beside_trees) = tree.children.split_at_mut(1);
        let content =
            self.content
                .as_widget_mut()
                .layout(&mut content_tree[0], renderer, &content_limits);
        let target = self.panel_target(content.size(), screen_room);
        let node_size = Size::new(target.width + gap_to_parent, target.y + target.height);
        let side = self.placement.side;
        let beside_nodes: Vec<Node> = self
            .beside
            .iter_mut()
            .zip(beside_trees.iter_mut())
            .scan(0.0, |placed_width: &mut f32, (panel, panel_tree)| {
                panel.parent_panel_top = target.y;
                let remaining_width = (room.width - node_size.width - *placed_width).max(0.0);
                let node = panel.layout(
                    panel_tree,
                    renderer,
                    &screen_room.limits_within(remaining_width),
                );
                let extent = drawn_extent(&node);
                let x = match side {
                    NotchSide::Left => node_size.width + *placed_width,
                    NotchSide::Right => -*placed_width - extent.width,
                };
                *placed_width += extent.width;
                Some(
                    Node::with_children(
                        extent.size(),
                        vec![node.move_to(Point::new(-extent.x, -extent.y))],
                    )
                    .move_to(Point::new(x, extent.y)),
                )
            })
            .collect();
        let from_notch_edge = self.node_origin_from_notch_edge(node_size.width);
        let edge_target = target + from_notch_edge;
        let panel_morph: &mut PanelMorph = tree.state.downcast_mut();
        panel_morph.laid_out = edge_target;
        panel_morph.laid_out_folded = self.folded_into_notch(target) + from_notch_edge;
        panel_morph.morph.follow(panel_morph.morphing_toward);
        Node::with_children(
            node_size,
            once(content.move_to(target.position()))
                .chain(beside_nodes)
                .collect(),
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
        let panel = self.drawn_panel(panel_morph, layout.bounds());
        if panel.width <= 0.0 || panel.height <= 0.0 {
            return;
        }
        let frame_style = container::Style {
            shadow: self.frame.lifted_drop_shadow(),
            ..self.frame.container_style(theme)
        };
        match self.placement.submenu_connector {
            SubmenuConnector::Shown => self.connector(panel, origin).draw_framed_surface(
                renderer,
                theme,
                ConnectedSurface {
                    frame: self.frame,
                    bounds: panel,
                    corners: self.panel_corners(),
                    shadow: frame_style.shadow,
                },
            ),
            SubmenuConnector::Hidden => container::draw_background(renderer, &frame_style, panel),
        }
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
        self.beside
            .iter()
            .zip(&tree.children[1..])
            .zip(
                layout
                    .children()
                    .skip(1)
                    .filter_map(|wrapper| wrapper.children().next()),
            )
            .for_each(|((panel, panel_tree), panel_layout)| {
                panel.draw(
                    panel_tree,
                    renderer,
                    theme,
                    style,
                    panel_layout,
                    cursor,
                    viewport,
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
        let surface_corners = match self.placement.submenu_connector {
            SubmenuConnector::Shown => Radius::default(),
            SubmenuConnector::Hidden => self.panel_corners(),
        };
        operation.custom(
            None,
            layout.bounds(),
            &mut NotchedPanelRegions {
                surface_rows: self.drawn_surface_rows(panel, origin),
                surface_corners,
                accepting_input: self.panel_with_gap(panel),
                bounds: layout.bounds(),
                nesting: self.nesting,
            },
        );
        let content_layout = layout.children().next().unwrap();
        let (content_tree, beside_trees) = tree.children.split_at_mut(1);
        self.content.as_widget_mut().operate(
            &mut content_tree[0],
            content_layout,
            renderer,
            operation,
        );
        self.beside
            .iter_mut()
            .zip(beside_trees.iter_mut())
            .zip(
                layout
                    .children()
                    .skip(1)
                    .filter_map(|wrapper| wrapper.children().next()),
            )
            .for_each(|((panel, panel_tree), panel_layout)| {
                panel.operate(panel_tree, panel_layout, renderer, operation);
            });
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
        let (content_tree, beside_trees) = tree.children.split_at_mut(1);
        self.content.as_widget_mut().update(
            &mut content_tree[0],
            event,
            content_layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        self.beside
            .iter_mut()
            .zip(beside_trees.iter_mut())
            .zip(
                layout
                    .children()
                    .skip(1)
                    .filter_map(|wrapper| wrapper.children().next()),
            )
            .for_each(|((panel, panel_tree), panel_layout)| {
                panel.update(
                    panel_tree,
                    event,
                    panel_layout,
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    viewport,
                );
            });
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };
        let panel_morph: &mut PanelMorph = tree.state.downcast_mut();
        if panel_morph.laid_out != panel_morph.morphing_toward {
            panel_morph.morphing_toward = panel_morph.laid_out;
            panel_morph.morph.begin(panel_morph.laid_out_folded);
        }
        panel_morph.morph.tick(*now, self.morph);
        panel_morph.morph.follow(panel_morph.morphing_toward);
        if panel_morph.morph.motion_state() == MotionState::Moving {
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
        once(self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content_layout,
            cursor,
            viewport,
            renderer,
        ))
        .chain(
            self.beside
                .iter()
                .zip(&tree.children[1..])
                .zip(
                    layout
                        .children()
                        .skip(1)
                        .filter_map(|wrapper| wrapper.children().next()),
                )
                .map(|((panel, panel_tree), panel_layout)| {
                    panel.mouse_interaction(panel_tree, panel_layout, cursor, viewport, renderer)
                }),
        )
        .max()
        .unwrap_or_default()
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

#[cfg(test)]
mod nesting_tests {
    use iced::{
        Element, Rectangle, Settings, Size,
        widget::{Column, text},
    };
    use iced_test::Simulator;

    use super::{NotchRowAnchor, notched_panel};
    use crate::popovers::{CascadeDirection, CascadeOrientation, PopupGrowth, cascade_row};

    const ROW_OFFSET: f32 = 30.0;

    fn cascade_with_card(
        direction: CascadeDirection,
        list_row_top: f32,
        card_label: &'static str,
    ) -> Element<'static, ()> {
        let orientation = CascadeOrientation {
            direction,
            growth: PopupGrowth::Downward,
        };
        let card = notched_panel(
            text(card_label),
            orientation,
            NotchRowAnchor::WithinParent {
                row_offset: ROW_OFFSET,
            },
        )
        .row_height(20.0);
        let list_rows = Column::with_children(
            ["list", "second", "third", "fourth", "fifth", "sixth"].map(|label| text(label).into()),
        );
        let list = notched_panel(
            list_rows,
            orientation,
            NotchRowAnchor::FromPrimaryTop(list_row_top),
        )
        .row_height(20.0)
        .beside([card]);
        cascade_row(text("primary").into(), [list.into()], orientation)
    }

    fn bounds_of(simulator: &mut Simulator<'_, ()>, label: &str) -> Rectangle {
        simulator
            .find(label)
            .unwrap_or_else(|_| panic!("{label} is laid out"))
            .bounds()
    }

    fn simulated(screen: Size, cascade: Element<'static, ()>) -> Simulator<'static, ()> {
        Simulator::with_size(Settings::default(), screen, cascade)
    }

    #[test]
    fn a_nested_card_lines_up_with_its_row_and_has_room() {
        let mut simulator = simulated(
            Size::new(600.0, 400.0),
            cascade_with_card(CascadeDirection::Rightward, 40.0, "card"),
        );
        let list = bounds_of(&mut simulator, "list");
        let card = bounds_of(&mut simulator, "card");
        assert!(
            (card.y - list.y - ROW_OFFSET).abs() < 0.5,
            "list {list:?}, card {card:?}"
        );
        assert!(card.x > list.x + list.width, "list {list:?}, card {card:?}");
        assert!(card.width > 0.0, "card {card:?}");
    }

    #[test]
    fn a_leftward_card_opens_left_of_its_list() {
        let mut simulator = simulated(
            Size::new(600.0, 400.0),
            cascade_with_card(CascadeDirection::Leftward, 40.0, "card"),
        );
        let list = bounds_of(&mut simulator, "list");
        let card = bounds_of(&mut simulator, "card");
        assert!(
            card.x + card.width <= list.x + 0.5,
            "list {list:?}, card {card:?}"
        );
        assert!(card.width > 0.0, "card {card:?}");
    }

    #[test]
    fn a_nested_card_follows_a_list_pushed_back_on_screen() {
        let mut simulator = simulated(
            Size::new(600.0, 120.0),
            cascade_with_card(CascadeDirection::Rightward, 110.0, "card"),
        );
        let list = bounds_of(&mut simulator, "list");
        let card = bounds_of(&mut simulator, "card");
        assert!(
            list.y < 110.0,
            "the list is pushed back on screen, at {list:?}"
        );
        assert!(
            (card.y - list.y - ROW_OFFSET).abs() < 0.5,
            "list {list:?}, card {card:?}"
        );
    }

    #[test]
    fn a_wider_card_leaves_the_list_width_unchanged() {
        let list_width = |card_label: &'static str| {
            let mut simulator = simulated(
                Size::new(600.0, 400.0),
                cascade_with_card(CascadeDirection::Leftward, 40.0, card_label),
            );
            bounds_of(&mut simulator, "sixth").width
        };
        assert!(
            (list_width("card") - list_width("a much much wider card than the list itself")).abs()
                < 0.5
        );
    }
}
