use std::iter::once;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
};

use crate::widgets::anchored_popup::AnchoredPopup;

type SheetWidget<'w, Message> = &'w dyn Widget<Message, iced::Theme, iced::Renderer>;

pub struct PointerCrossing<Message> {
    pub entered: Message,
    pub left: Message,
}

pub fn anchored_sheet<'a, Message: Clone + 'a>(
    handle: impl Into<Element<'a, Message>>,
    sheet: impl IntoIterator<Item = AnchoredPopup<'a, Message>>,
    float_offset_px: f32,
    crossing: PointerCrossing<Message>,
) -> Element<'a, Message> {
    Element::new(AnchoredSheet {
        handle: handle.into(),
        sheet: sheet.into_iter().collect(),
        float_offset_px,
        crossing,
    })
}

struct AnchoredSheet<'a, Message> {
    handle: Element<'a, Message>,
    sheet: Vec<AnchoredPopup<'a, Message>>,
    float_offset_px: f32,
    crossing: PointerCrossing<Message>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct PointerOver {
    handle: bool,
    sheet: bool,
}

impl PointerOver {
    fn either(self) -> bool {
        self.handle || self.sheet
    }
}

fn publish_crossing<Message: Clone>(
    before: PointerOver,
    after: PointerOver,
    crossing: &PointerCrossing<Message>,
    shell: &mut Shell<'_, Message>,
) {
    match (before.either(), after.either()) {
        (false, true) => shell.publish(crossing.entered.clone()),
        (true, false) => shell.publish(crossing.left.clone()),
        _ => {}
    }
}

impl<'a, Message: Clone + 'a> AnchoredSheet<'a, Message> {
    fn child_widgets(&self) -> Vec<SheetWidget<'_, Message>> {
        once(self.handle.as_widget())
            .chain(
                self.sheet
                    .iter()
                    .map(|popup| popup as SheetWidget<'_, Message>),
            )
            .collect()
    }
}

impl<'a, Message: Clone + 'a> Widget<Message, iced::Theme, iced::Renderer>
    for AnchoredSheet<'a, Message>
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<PointerOver>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(PointerOver::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.child_widgets().into_iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.child_widgets());
    }

    fn size(&self) -> Size<Length> {
        self.handle.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.handle
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
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.handle.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.handle
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
        if let Event::Mouse(_) = event {
            let pointer = tree.state.downcast_mut::<PointerOver>();
            let before = *pointer;
            pointer.handle = cursor.is_over(layout.bounds());
            pointer.sheet &= !self.sheet.is_empty();
            publish_crossing(before, *pointer, &self.crossing, shell);
        }
        self.handle.as_widget_mut().update(
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

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.handle.as_widget().mouse_interaction(
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
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let anchor = layout.bounds() + translation;
        let viewport = *viewport;
        let float_offset_px = self.float_offset_px;
        let crossing = &self.crossing;
        let pointer = tree.state.downcast_mut::<PointerOver>();
        let (handle_tree, sheet_tree) = tree.children.split_at_mut(1);
        let handle_overlay = self.handle.as_widget_mut().overlay(
            &mut handle_tree[0],
            layout,
            renderer,
            &viewport,
            translation,
        );
        let sheet_overlay =
            self.sheet
                .first_mut()
                .zip(sheet_tree.first_mut())
                .map(|(popup, popup_tree)| {
                    popup.reanchor(anchor - Vector::new(float_offset_px, float_offset_px));
                    overlay::Element::new(Box::new(SheetOverlay {
                        popup,
                        tree: popup_tree,
                        pointer,
                        viewport,
                        float_offset_px,
                        crossing,
                    }))
                });
        let overlays: Vec<_> = handle_overlay.into_iter().chain(sheet_overlay).collect();
        (!overlays.is_empty()).then(|| overlay::Group::with_children(overlays).overlay())
    }
}

struct SheetOverlay<'a, 'b, Message> {
    popup: &'b mut AnchoredPopup<'a, Message>,
    tree: &'b mut Tree,
    pointer: &'b mut PointerOver,
    viewport: Rectangle,
    float_offset_px: f32,
    crossing: &'b PointerCrossing<Message>,
}

impl<Message: Clone> overlay::Overlay<Message, iced::Theme, iced::Renderer>
    for SheetOverlay<'_, '_, Message>
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        let inset = 2.0 * self.float_offset_px;
        let inner = Size::new(bounds.width - inset, bounds.height - inset);
        self.popup
            .layout(self.tree, renderer, &Limits::new(inner, inner))
            .move_to((self.float_offset_px, self.float_offset_px))
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        self.popup.draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &self.viewport,
        );
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.popup.operate(self.tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Event::Mouse(_) = event {
            let before = *self.pointer;
            self.pointer.sheet =
                cursor.is_over(AnchoredPopup::<Message>::popup_bounds(self.tree, layout));
            publish_crossing(before, *self.pointer, self.crossing, shell);
        }
        let viewport = self.viewport;
        self.popup.update(
            self.tree, event, layout, cursor, renderer, clipboard, shell, &viewport,
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.popup
            .mouse_interaction(self.tree, layout, cursor, &self.viewport, renderer)
    }

    fn overlay<'c>(
        &'c mut self,
        layout: Layout<'c>,
        renderer: &iced::Renderer,
    ) -> Option<overlay::Element<'c, Message, iced::Theme, iced::Renderer>> {
        let viewport = self.viewport;
        self.popup
            .overlay(self.tree, layout, renderer, &viewport, Vector::ZERO)
    }
}
