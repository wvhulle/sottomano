use std::iter::once;

use corredo::{
    popovers::{CascadeDirection, CascadeOrientation, PopupGrowth},
    width_layout::fair_column_widths,
};
use iced::{
    Element, Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse::{Cursor, Interaction},
};
use sottomano_toml_config::screen_edges::Position;

use crate::group_menus::cascading_layout::notched_panel::ScreenRoom;

#[must_use]
pub fn growth_away_from(bar: Position) -> PopupGrowth {
    match bar {
        Position::Bottom => PopupGrowth::Upward,
        Position::Top | Position::Left => PopupGrowth::Downward,
    }
}

pub struct CascadeRow<'a, Message> {
    panels: Vec<Element<'a, Message>>,
    orientation: CascadeOrientation,
}

fn drawn_extent(panel: &Node) -> Rectangle {
    panel.children().iter().map(Node::bounds).fold(
        Rectangle::new(Point::ORIGIN, panel.size()),
        |extent, child| extent.union(&child),
    )
}

#[must_use]
pub fn cascade_row<'a, Message: 'a>(
    primary: Element<'a, Message>,
    satellites: impl IntoIterator<Item = Element<'a, Message>>,
    orientation: CascadeOrientation,
) -> Element<'a, Message> {
    Element::new(CascadeRow {
        panels: once(primary).chain(satellites).collect(),
        orientation,
    })
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for CascadeRow<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.panels.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.panels);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let available = limits.max();
        let (primary_slot, satellite_slots) = self.panels.split_at_mut(1);
        let (primary_tree, satellite_trees) = tree.children.split_at_mut(1);
        let primary =
            primary_slot[0]
                .as_widget_mut()
                .layout(&mut primary_tree[0], renderer, limits);
        let room_width = available.width - primary.size().width;
        let screen_room = match self.orientation.growth {
            PopupGrowth::Downward => ScreenRoom {
                above_primary_top: 0.0,
                below_primary_top: available.height,
            },
            PopupGrowth::Upward => ScreenRoom {
                above_primary_top: available.height - primary.size().height,
                below_primary_top: primary.size().height,
            },
        };
        let within = |width: f32| screen_room.limits_within(width.max(0.0));
        let natural_widths: Vec<f32> = satellite_slots
            .iter_mut()
            .zip(satellite_trees.iter_mut())
            .map(|(satellite, satellite_tree)| {
                satellite
                    .as_widget_mut()
                    .layout(satellite_tree, renderer, &within(room_width))
                    .size()
                    .width
            })
            .collect();
        let satellites: Vec<Node> = satellite_slots
            .iter_mut()
            .zip(satellite_trees.iter_mut())
            .zip(fair_column_widths(&natural_widths, room_width))
            .map(|((satellite, satellite_tree), allotted_width)| {
                satellite
                    .as_widget_mut()
                    .layout(satellite_tree, renderer, &within(allotted_width))
            })
            .collect();
        let nodes: Vec<Node> = once(primary).chain(satellites).collect();
        let total_width: f32 = nodes.iter().map(|node| node.size().width).sum();
        let overhang_above = nodes
            .iter()
            .map(|node| -drawn_extent(node).y)
            .fold(0.0, f32::max);
        let height = nodes
            .iter()
            .map(|node| {
                let extent = drawn_extent(node);
                extent.y + extent.height
            })
            .fold(0.0, f32::max)
            + overhang_above;
        let placed = nodes
            .into_iter()
            .scan(0.0, |extent_so_far: &mut f32, node| {
                let width = node.size().width;
                let x = match self.orientation.direction {
                    CascadeDirection::Rightward => *extent_so_far,
                    CascadeDirection::Leftward => total_width - *extent_so_far - width,
                };
                *extent_so_far += width;
                Some(node.move_to(Point::new(x, overhang_above)))
            })
            .collect();
        Node::with_children(Size::new(total_width, height), placed)
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
        self.panels
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((panel, panel_tree), panel_layout)| {
                panel.as_widget_mut().update(
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
        self.panels
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .for_each(|((panel, panel_tree), panel_layout)| {
                panel.as_widget().draw(
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

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> Interaction {
        self.panels
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((panel, panel_tree), panel_layout)| {
                panel.as_widget().mouse_interaction(
                    panel_tree,
                    panel_layout,
                    cursor,
                    viewport,
                    renderer,
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.panels
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .for_each(|((panel, panel_tree), panel_layout)| {
                    panel
                        .as_widget_mut()
                        .operate(panel_tree, panel_layout, renderer, operation);
                });
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        overlay::from_children(
            &mut self.panels,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
