use core::slice;
use std::time::{Duration, Instant};

use iced::{
    Border, Color, Element, Event, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    border::Radius,
    mouse::{Cursor, Interaction},
};

const FLASH: Duration = Duration::from_millis(600);
const FLASH_ALPHA: f32 = 0.45;
const FLASH_RADIUS_PX: f32 = 3.0;

pub fn copyable<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    text: String,
    on_copy: impl Fn(String) -> Message + 'a,
    flash: Color,
) -> Element<'a, Message> {
    Element::new(Copyable {
        content: content.into(),
        text,
        on_copy: Box::new(on_copy),
        flash,
    })
}

struct Copyable<'a, Message> {
    content: Element<'a, Message>,
    text: String,
    on_copy: Box<dyn Fn(String) -> Message + 'a>,
    flash: Color,
}

#[derive(Default)]
struct Copied(Option<Instant>);

impl Copied {
    fn fading(&self, now: Instant) -> Option<f32> {
        let elapsed = now.duration_since(self.0?);
        (elapsed < FLASH).then(|| 1.0 - elapsed.div_duration_f32(FLASH))
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for Copyable<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Copied>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Copied::default())
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

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let child = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(child.size(), vec![child])
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if shell.is_event_captured() {
            return;
        }
        let pressed = matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        );
        if !pressed || !cursor.is_over(layout.bounds()) {
            return;
        }
        let now = Instant::now();
        tree.state.downcast_mut::<Copied>().0 = Some(now);
        shell.publish((self.on_copy)(self.text.clone()));
        shell.capture_event();
        shell.request_redraw_at(now + FLASH);
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
        if let Some(remaining) = tree.state.downcast_ref::<Copied>().fading(Instant::now()) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        radius: Radius::new(FLASH_RADIUS_PX),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                self.flash.scale_alpha(remaining * FLASH_ALPHA),
            );
        }
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
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
    ) -> Interaction {
        if cursor.is_over(layout.bounds()) {
            return Interaction::Copy;
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
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
        self.content.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
