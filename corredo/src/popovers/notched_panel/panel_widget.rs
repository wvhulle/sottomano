use std::iter::once;

use iced::{
    Event, Length, Point, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::Cursor,
    widget::container,
    window,
};

use super::{NotchSide, NotchedPanel, NotchedPanelRegions, PanelMorph, ScreenRoom};
use crate::{
    popovers::{ConnectedSurface, SubmenuConnector, cascade_row::drawn_extent},
    transitions::MotionState,
};

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
                let horizontal_offset = match side {
                    NotchSide::Left => node_size.width + *placed_width,
                    NotchSide::Right => -*placed_width - extent.width,
                };
                *placed_width += extent.width;
                Some(
                    Node::with_children(
                        extent.size(),
                        vec![node.move_to(Point::new(-extent.x, -extent.y))],
                    )
                    .move_to(Point::new(horizontal_offset, extent.y)),
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
                role: self.role,
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
