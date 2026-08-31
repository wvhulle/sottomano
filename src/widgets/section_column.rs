use core::slice;

use iced::{
    Color, Element, Event, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse::{Cursor, Interaction},
};

pub fn section_column<'a, Message: 'a>(
    sections: impl Into<Element<'a, Message>>,
    divider: Color,
    thickness: f32,
    overhang: f32,
) -> Element<'a, Message> {
    Element::new(SectionColumn {
        sections: sections.into(),
        divider,
        thickness,
        overhang,
    })
}

struct SectionColumn<'a, Message> {
    sections: Element<'a, Message>,
    divider: Color,
    thickness: f32,
    overhang: f32,
}

impl<Message> SectionColumn<'_, Message> {
    fn divider_bounds(
        &self,
        container: Rectangle,
        above: Rectangle,
        below: Rectangle,
    ) -> Rectangle {
        let gap_center = (above.y + above.height + below.y) / 2.0;
        Rectangle {
            x: container.x - self.overhang,
            y: gap_center - self.thickness / 2.0,
            width: container.width + self.overhang * 2.0,
            height: self.thickness,
        }
    }
}

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for SectionColumn<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.sections)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.sections));
    }

    fn size(&self) -> Size<Length> {
        self.sections.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let sections =
            self.sections
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(sections.size(), vec![sections])
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
        self.sections.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
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
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let sections = layout.children().next().unwrap();
        self.sections.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            sections,
            cursor,
            viewport,
        );

        let container = layout.bounds();
        let mut above: Option<Rectangle> = None;
        for section in sections.children() {
            let below = section.bounds();
            if let Some(above) = above {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: self.divider_bounds(container, above, below),
                        snap: false,
                        ..renderer::Quad::default()
                    },
                    self.divider,
                );
            }
            above = Some(below);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> Interaction {
        self.sections.as_widget().mouse_interaction(
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
        self.sections.as_widget_mut().operate(
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
        self.sections.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
