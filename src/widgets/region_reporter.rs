use core::slice;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::Cursor,
    window,
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RegionRoles {
    pub takes_input: bool,
    pub blurred: bool,
    pub corner_radius: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReportedRegion {
    pub rects: Vec<Rectangle>,
    pub roles: RegionRoles,
}

pub fn region_reporter<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    roles: RegionRoles,
    on_report: impl Fn(ReportedRegion) -> Message + 'a,
) -> RegionReporter<'a, Message> {
    RegionReporter {
        content: content.into(),
        roles,
        on_report: Box::new(on_report),
    }
}

pub struct RegionReporter<'a, Message> {
    content: Element<'a, Message>,
    roles: RegionRoles,
    on_report: Box<dyn Fn(ReportedRegion) -> Message + 'a>,
}

impl<'a, Message: 'a> From<RegionReporter<'a, Message>> for Element<'a, Message> {
    fn from(reporter: RegionReporter<'a, Message>) -> Self {
        Element::new(reporter)
    }
}

#[derive(Default)]
struct LastReport(Option<ReportedRegion>);

impl<Message> Widget<Message, iced::Theme, iced::Renderer> for RegionReporter<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<LastReport>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(LastReport::default())
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
        if matches!(event, Event::Window(window::Event::RedrawRequested(_))) {
            let region = ReportedRegion {
                rects: layout
                    .children()
                    .next()
                    .unwrap()
                    .children()
                    .map(|child| child.bounds())
                    .collect(),
                roles: self.roles,
            };
            let last = tree.state.downcast_mut::<LastReport>();
            if last.0.as_ref() != Some(&region) {
                last.0 = Some(region.clone());
                shell.publish((self.on_report)(region));
            }
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}
