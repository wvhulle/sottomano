mod hover_state_reports;

use std::slice::from_ref;

use corredo::popovers::NestedMenuOpenTrigger;
use iced::{
    Element, Event, Length, Rectangle, Size, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree, tree},
    },
};

use self::hover_state_reports::HoverState;
use crate::hover_cards::{HoverCardMessage, HoverTarget, PointerPresence};

pub fn hoverable<'view, M: 'view>(
    opening: NestedMenuOpenTrigger,
    content: Element<'view, M>,
    target: HoverTarget,
    wrap: impl Fn(HoverCardMessage) -> M + 'view,
) -> Element<'view, M> {
    Element::new(HoverArea {
        content,
        target,
        opening,
        wrap: Box::new(wrap),
    })
}

struct HoverArea<'view, M> {
    content: Element<'view, M>,
    target: HoverTarget,
    opening: NestedMenuOpenTrigger,
    wrap: Box<dyn Fn(HoverCardMessage) -> M + 'view>,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for HoverArea<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HoverState::from(self.target.clone()))
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
        if let Some(exit) = self.exit_of_replaced_target(state) {
            shell.publish(exit);
        }
        let anchor = layout.bounds();
        let pointer_presence = if cursor.is_over(anchor) {
            PointerPresence::PointedAt
        } else {
            PointerPresence::NotPointedAt
        };
        if let Some(laid_out) = self.laid_out_anchor(state, pointer_presence, anchor, viewport) {
            shell.publish(laid_out);
        }
        if self.opening == NestedMenuOpenTrigger::Press
            && pointer_presence == PointerPresence::PointedAt
            && !shell.is_event_captured()
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            )
        {
            state.pointer_presence = PointerPresence::PointedAt;
            shell.publish(self.menu_handle_pressed());
            shell.capture_event();
            return;
        }
        if let Some(message) = self.on_hover_change(state, pointer_presence) {
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
        if self.opening == NestedMenuOpenTrigger::Press && cursor.is_over(layout.bounds()) {
            content_interaction.max(mouse::Interaction::Pointer)
        } else {
            content_interaction
        }
    }

    fn overlay<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'tree, M, iced::Theme, iced::Renderer>> {
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
    use corredo::popovers::NestedMenuOpenTrigger;
    use iced::{
        Point, Settings, Size,
        mouse::{Button, Event as MouseEvent},
        widget::{Space, container},
    };
    use iced_test::Simulator;

    use super::hoverable;
    use crate::hover_cards::{AnchorEvent, HoverCardMessage, HoverTarget, PointerPresence};

    const INSIDE_AREA: Point = Point::new(10.0, 10.0);
    const OUTSIDE_AREA: Point = Point::new(100.0, 100.0);

    fn press_toggled_area<'view>() -> iced::Element<'view, HoverCardMessage> {
        hoverable(
            NestedMenuOpenTrigger::Press,
            container(Space::new()).width(40.0).height(40.0).into(),
            HoverTarget::Workspace(7),
            |message| message,
        )
    }

    fn hovered_area<'view>() -> iced::Element<'view, HoverCardMessage> {
        hoverable(
            NestedMenuOpenTrigger::Hover,
            container(Space::new()).width(40.0).height(40.0).into(),
            HoverTarget::Workspace(7),
            |message| message,
        )
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum PointerButtonAction {
        Hover,
        Press,
    }

    fn messages_from_pointing_inside(
        area: iced::Element<'_, HoverCardMessage>,
        pointer_button: PointerButtonAction,
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
        if pointer_button == PointerButtonAction::Press {
            let _pressed =
                simulator.simulate([iced::Event::Mouse(MouseEvent::ButtonPressed(Button::Left))]);
        }
        simulator.into_messages().collect()
    }

    #[test]
    fn hovering_a_menu_handle_reports_it_without_entering() {
        let messages =
            messages_from_pointing_inside(press_toggled_area(), PointerButtonAction::Hover);
        assert!(
            matches!(
                messages.as_slice(),
                [
                    HoverCardMessage::Anchor(AnchorEvent::LaidOut(..)),
                    HoverCardMessage::Anchor(AnchorEvent::HandlePointed {
                        pointing: PointerPresence::PointedAt,
                        ..
                    })
                ]
            ),
            "{messages:?}"
        );
    }

    #[test]
    fn pressing_a_menu_handle_reports_the_press_after_its_layout() {
        let messages =
            messages_from_pointing_inside(press_toggled_area(), PointerButtonAction::Press);
        assert!(
            matches!(
                messages.as_slice(),
                [
                    HoverCardMessage::Anchor(AnchorEvent::LaidOut(..)),
                    HoverCardMessage::Anchor(AnchorEvent::HandlePointed { .. }),
                    HoverCardMessage::Anchor(AnchorEvent::HandlePressed { .. })
                ]
            ),
            "{messages:?}"
        );
    }

    #[test]
    fn hovering_a_hover_area_opens_its_card() {
        let messages = messages_from_pointing_inside(hovered_area(), PointerButtonAction::Hover);
        assert!(
            matches!(
                messages.as_slice(),
                [
                    HoverCardMessage::Anchor(AnchorEvent::LaidOut(..)),
                    HoverCardMessage::Anchor(AnchorEvent::Entered { .. })
                ]
            ),
            "{messages:?}"
        );
    }
}
