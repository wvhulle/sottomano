use std::slice;

use iced::{
    Element, Event, Length, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    time::Instant,
    window,
};

use crate::transitions::MotionState;

pub(super) struct PaintedRow<'state, PainterState> {
    pub(super) painter_state: &'state PainterState,
    pub(super) child_bounds: &'state [Rectangle],
    pub(super) row_bounds: Rectangle,
}

pub(super) trait SlotPainter {
    type PainterState: Default + 'static;

    fn advance(
        &self,
        _painter_state: &mut Self::PainterState,
        _child_bounds: &[Rectangle],
        _cursor: Cursor,
        _now: Instant,
    ) -> MotionState {
        MotionState::Settled
    }

    fn pointer_motion(
        &self,
        _painter_state: &Self::PainterState,
        _child_bounds: &[Rectangle],
        _cursor: Cursor,
    ) -> MotionState {
        MotionState::Settled
    }

    fn draw_under(
        &self,
        painted_row: PaintedRow<'_, Self::PainterState>,
        renderer: &mut iced::Renderer,
        theme: &Theme,
    );

    fn draw_over(
        &self,
        _painted_row: PaintedRow<'_, Self::PainterState>,
        _renderer: &mut iced::Renderer,
        _theme: &Theme,
    ) {
    }
}

pub(super) struct SlotUnderlay<'view, Message, Painter> {
    painter: Painter,
    content: Element<'view, Message>,
}

impl<'view, Message, Painter> SlotUnderlay<'view, Message, Painter> {
    pub(super) fn new(painter: Painter, content: impl Into<Element<'view, Message>>) -> Self {
        Self {
            painter,
            content: content.into(),
        }
    }
}

fn layout_child_bounds(layout: Layout<'_>) -> Vec<Rectangle> {
    layout.children().map(|child| child.bounds()).collect()
}

impl<Message, Painter: SlotPainter> Widget<Message, Theme, iced::Renderer>
    for SlotUnderlay<'_, Message, Painter>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Painter::PainterState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Painter::PainterState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
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
        let painter_motion = match event {
            Event::Window(window::Event::RedrawRequested(now)) => self.painter.advance(
                tree.state.downcast_mut::<Painter::PainterState>(),
                &layout_child_bounds(layout),
                cursor,
                *now,
            ),
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft) => {
                self.painter.pointer_motion(
                    tree.state.downcast_ref::<Painter::PainterState>(),
                    &layout_child_bounds(layout),
                    cursor,
                )
            }
            _ => MotionState::Settled,
        };
        if painter_motion == MotionState::Moving {
            shell.request_redraw();
        }
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
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let painter_state = tree.state.downcast_ref::<Painter::PainterState>();
        let child_bounds = layout_child_bounds(layout);
        let painted_row = || PaintedRow {
            painter_state,
            child_bounds: &child_bounds,
            row_bounds: layout.bounds(),
        };
        self.painter.draw_under(painted_row(), renderer, theme);
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        self.painter.draw_over(painted_row(), renderer, theme);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
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

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'view, Message: 'view, Painter: SlotPainter + 'view>
    From<SlotUnderlay<'view, Message, Painter>> for Element<'view, Message>
{
    fn from(slot_underlay: SlotUnderlay<'view, Message, Painter>) -> Self {
        Element::new(slot_underlay)
    }
}
