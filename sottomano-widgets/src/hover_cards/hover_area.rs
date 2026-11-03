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
    group_menus::group_menu_state::GroupMenuPin,
    hover_cards::{
        HoverAnnouncement, HoverCardMessage, HoverTarget, MenuHandleGesture, PressHints,
    },
};

pub fn hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    press_hints: PressHints,
    wrap: impl Fn(HoverCardMessage) -> M + 'a,
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
    wrap: impl Fn(HoverCardMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        announcement,
        opening: CardOpening::OnHover,
        wrap: Box::new(wrap),
    })
}

pub fn press_toggled_hoverable<'a, M: 'a>(
    content: Element<'a, M>,
    target: HoverTarget,
    announcement: HoverAnnouncement,
    wrap: impl Fn(HoverCardMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        announcement,
        opening: CardOpening::OnPress,
        wrap: Box::new(wrap),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardOpening {
    OnHover,
    OnPress,
}

struct HoverArea<'a, M> {
    content: Element<'a, M>,
    target: HoverTarget,
    announcement: HoverAnnouncement,
    opening: CardOpening,
    wrap: Box<dyn Fn(HoverCardMessage) -> M + 'a>,
}

#[derive(Default)]
struct HoverState {
    hovered: bool,
    announced: HoverAnnouncement,
    reported_anchor: Rectangle,
}

impl<M> HoverArea<'_, M> {
    fn enter(&self, state: &mut HoverState, anchor: Rectangle) -> M {
        state.announced = self.announcement.clone();
        (self.wrap)(HoverCardMessage::EnterAnchor {
            target: self.target.clone(),
            anchor,
            announcement: self.announcement.clone(),
        })
    }

    fn menu_handle(&self, gesture: MenuHandleGesture, anchor: Rectangle) -> M {
        (self.wrap)(HoverCardMessage::MenuHandleAnchor {
            gesture,
            target: self.target.clone(),
            anchor,
            announcement: self.announcement.clone(),
        })
    }

    fn exit(&self) -> M {
        (self.wrap)(HoverCardMessage::Exit {
            target: self.target.clone(),
        })
    }

    fn revised_announcement(&self, state: &mut HoverState) -> Option<M> {
        if !state.hovered || state.announced == self.announcement {
            return None;
        }
        state.announced = self.announcement.clone();
        Some((self.wrap)(HoverCardMessage::AnnouncementChanged {
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
        match (hovered, self.opening) {
            (true, CardOpening::OnHover) => Some(self.enter(state, anchor)),
            (true, CardOpening::OnPress) => {
                Some(self.menu_handle(MenuHandleGesture::Hovered, anchor))
            }
            (false, _) => Some(self.exit()),
        }
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
        if self.opening == CardOpening::OnPress && state.reported_anchor != anchor {
            state.reported_anchor = anchor;
            shell.publish(self.menu_handle(MenuHandleGesture::LaidOut, anchor));
        }
        if self.opening == CardOpening::OnPress
            && hovered
            && !shell.is_event_captured()
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            )
        {
            state.hovered = true;
            shell.publish(self.menu_handle(MenuHandleGesture::Pressed, anchor));
            shell.capture_event();
            return;
        }
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
        let content_interaction = self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        );
        if self.opening == CardOpening::OnPress && cursor.is_over(layout.bounds()) {
            content_interaction.max(mouse::Interaction::Pointer)
        } else {
            content_interaction
        }
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
    entry: GroupMenuPin,
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

#[cfg(test)]
mod tests {
    use iced::{
        Point, Settings, Size,
        mouse::{Button, Event as MouseEvent},
        widget::{Space, container},
    };
    use iced_test::Simulator;

    use super::{announcing_hoverable, press_toggled_hoverable};
    use crate::hover_cards::{HoverAnnouncement, HoverCardMessage, HoverTarget, MenuHandleGesture};

    const INSIDE_AREA: Point = Point::new(10.0, 10.0);
    const OUTSIDE_AREA: Point = Point::new(100.0, 100.0);

    fn press_toggled_area<'a>() -> iced::Element<'a, HoverCardMessage> {
        press_toggled_hoverable(
            container(Space::new()).width(40.0).height(40.0).into(),
            HoverTarget::Workspace(7),
            HoverAnnouncement::default(),
            |message| message,
        )
    }

    fn hovered_area<'a>() -> iced::Element<'a, HoverCardMessage> {
        announcing_hoverable(
            container(Space::new()).width(40.0).height(40.0).into(),
            HoverTarget::Workspace(7),
            HoverAnnouncement::default(),
            |message| message,
        )
    }

    fn messages_from_pointing_inside(
        area: iced::Element<'_, HoverCardMessage>,
        pressed: bool,
    ) -> Vec<HoverCardMessage> {
        let mut simulator =
            Simulator::with_size(Settings::default(), Size::new(200.0, 200.0), area);
        simulator.point_at(OUTSIDE_AREA);
        let _moved_outside = simulator.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
            position: OUTSIDE_AREA,
        })]);
        simulator.point_at(INSIDE_AREA);
        let _moved_inside = simulator.simulate([iced::Event::Mouse(MouseEvent::CursorMoved {
            position: INSIDE_AREA,
        })]);
        if pressed {
            let _pressed =
                simulator.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
        }
        simulator.into_messages().collect()
    }

    fn handle_gestures(messages: &[HoverCardMessage]) -> Vec<MenuHandleGesture> {
        messages
            .iter()
            .filter_map(|message| match message {
                HoverCardMessage::MenuHandleAnchor { gesture, .. } => Some(*gesture),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn hovering_a_menu_handle_reports_it_without_entering() {
        let messages = messages_from_pointing_inside(press_toggled_area(), false);
        assert_eq!(
            handle_gestures(&messages),
            [MenuHandleGesture::LaidOut, MenuHandleGesture::Hovered]
        );
        assert_eq!(messages.len(), 2, "{messages:?}");
    }

    #[test]
    fn pressing_a_menu_handle_reports_the_press() {
        let messages = messages_from_pointing_inside(press_toggled_area(), true);
        assert_eq!(
            handle_gestures(&messages),
            [
                MenuHandleGesture::LaidOut,
                MenuHandleGesture::Hovered,
                MenuHandleGesture::Pressed
            ]
        );
    }

    #[test]
    fn hovering_a_hover_area_opens_its_card() {
        let messages = messages_from_pointing_inside(hovered_area(), false);
        assert!(
            matches!(messages.as_slice(), [HoverCardMessage::EnterAnchor { .. }]),
            "{messages:?}"
        );
    }
}
