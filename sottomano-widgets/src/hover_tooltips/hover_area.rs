use std::slice::from_ref;

use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
};

use crate::{
    hover_tooltips::{HoverAnnouncement, HoverTarget, PressHints, TooltipMessage},
    widget_groups::group_menus::PinnedEntry,
};

pub fn hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    press_hints: PressHints,
    wrap: impl Fn(TooltipMessage) -> M + 'a,
) -> Element<'a, M> {
    announcing_hoverable(
        content,
        target,
        HoverAnnouncement::from_reading(press_hints),
        wrap,
    )
}

pub fn announcing_hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    announcement: HoverAnnouncement,
    wrap: impl Fn(TooltipMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        announcement,
        wrap: Box::new(wrap),
    })
}

struct HoverArea<'a, M> {
    content: Element<'a, M>,
    target: HoverTarget,
    announcement: HoverAnnouncement,
    wrap: Box<dyn Fn(TooltipMessage) -> M + 'a>,
}

#[derive(Default)]
struct HoverState {
    hovered: bool,
    announced: HoverAnnouncement,
}

impl<M> HoverArea<'_, M> {
    fn enter(&self, state: &mut HoverState, anchor: Rectangle) -> M {
        state.announced = self.announcement.clone();
        (self.wrap)(TooltipMessage::EnterAnchor {
            target: self.target.clone(),
            anchor,
            announcement: self.announcement.clone(),
        })
    }

    fn exit(&self) -> M {
        (self.wrap)(TooltipMessage::Exit {
            target: self.target.clone(),
        })
    }

    fn revised_announcement(&self, state: &mut HoverState) -> Option<M> {
        if !state.hovered || state.announced == self.announcement {
            return None;
        }
        state.announced = self.announcement.clone();
        Some((self.wrap)(TooltipMessage::AnnouncementChanged {
            target: self.target.clone(),
            announcement: self.announcement.clone(),
        }))
    }

    fn on_hover_change(
        &self,
        state: &mut HoverState,
        hovered: bool,
        anchor: Rectangle,
    ) -> Option<M> {
        if hovered == state.hovered {
            return None;
        }
        state.hovered = hovered;
        Some(if hovered {
            self.enter(state, anchor)
        } else {
            self.exit()
        })
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for HoverArea<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HoverState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
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
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
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

        let state: &mut HoverState = tree.state.downcast_mut();
        let anchor = layout.bounds();
        let hovered = cursor.is_over(anchor);
        let published = self
            .on_hover_change(state, hovered, anchor)
            .or_else(|| self.revised_announcement(state));
        if let Some(message) = published {
            shell.publish(message);
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.custom(None, layout.bounds(), &mut self.target);
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
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

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[must_use]
pub fn marked_by_hover_target<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
) -> Element<'a, M> {
    Element::new(LocatableMark {
        content,
        marker: target,
    })
}

#[must_use]
pub fn marked_by_menu_pin<'a, M: 'a>(
    content: Element<'a, M>,
    entry: PinnedEntry,
) -> Element<'a, M> {
    Element::new(LocatableMark {
        content,
        marker: entry,
    })
}

struct LocatableMark<'a, M, Marker> {
    content: Element<'a, M>,
    marker: Marker,
}

impl<M, Marker: Send + Sync + 'static> Widget<M, iced::Theme, iced::Renderer>
    for LocatableMark<'_, M, Marker>
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.content
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
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
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

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.custom(None, layout.bounds(), &mut self.marker);
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
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

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
