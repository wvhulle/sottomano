use std::{mem::replace, slice::from_ref, sync::Arc};

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

use crate::hover_cards::{
    AnchorEvent, HoverCardMessage, HoverTarget, PointerPresence, WidgetAnchor,
};

pub fn hoverable<'a, M: 'a>(
    opening: NestedMenuOpenTrigger,
    content: Element<'a, M>,
    target: HoverTarget,
    wrap: impl Fn(HoverCardMessage) -> M + 'a,
) -> Element<'a, M> {
    Element::new(HoverArea {
        content,
        target,
        opening,
        wrap: Box::new(wrap),
    })
}

struct HoverArea<'a, M> {
    content: Element<'a, M>,
    target: HoverTarget,
    opening: NestedMenuOpenTrigger,
    wrap: Box<dyn Fn(HoverCardMessage) -> M + 'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AnchorReport {
    Unreported,
    Reported { anchor: Rectangle, visible: bool },
}

struct HoverState {
    target: HoverTarget,
    pointer_presence: PointerPresence,
    anchor_report: AnchorReport,
    presence: Arc<()>,
}

impl From<HoverTarget> for HoverState {
    fn from(target: HoverTarget) -> Self {
        Self {
            target,
            pointer_presence: PointerPresence::NotPointedAt,
            anchor_report: AnchorReport::Unreported,
            presence: Arc::new(()),
        }
    }
}

impl<M> HoverArea<'_, M> {
    fn exit_of_replaced_target(&self, state: &mut HoverState) -> Option<M> {
        if state.target == self.target {
            return None;
        }
        let replaced = replace(state, HoverState::from(self.target.clone()));
        (replaced.pointer_presence == PointerPresence::PointedAt).then(|| {
            (self.wrap)(match self.opening {
                NestedMenuOpenTrigger::Hover => HoverCardMessage::Exit {
                    target: replaced.target,
                },
                NestedMenuOpenTrigger::Press => {
                    HoverCardMessage::Anchor(AnchorEvent::HandlePointed {
                        target: replaced.target,
                        pointing: PointerPresence::NotPointedAt,
                    })
                }
            })
        })
    }

    fn laid_out_anchor(
        &self,
        state: &mut HoverState,
        pointer_presence: PointerPresence,
        anchor: Rectangle,
        viewport: &Rectangle,
    ) -> Option<M> {
        let pointed_at = pointer_presence == PointerPresence::PointedAt;
        let reports = self.opening == NestedMenuOpenTrigger::Press
            || pointed_at
            || state.anchor_report != AnchorReport::Unreported;
        let visible = pointed_at || anchor.intersection(viewport).is_some();
        let anchor_report = AnchorReport::Reported { anchor, visible };
        if !reports || state.anchor_report == anchor_report {
            return None;
        }
        state.anchor_report = anchor_report;
        Some((self.wrap)(HoverCardMessage::Anchor(AnchorEvent::LaidOut(
            WidgetAnchor {
                target: self.target.clone(),
                opening: self.opening,
                anchor,
                visible,
                presence: Arc::downgrade(&state.presence),
            },
        ))))
    }

    fn enter(&self) -> M {
        (self.wrap)(HoverCardMessage::Anchor(AnchorEvent::Entered {
            target: self.target.clone(),
        }))
    }

    fn menu_handle_pointed(&self, pointing: PointerPresence) -> M {
        (self.wrap)(HoverCardMessage::Anchor(AnchorEvent::HandlePointed {
            target: self.target.clone(),
            pointing,
        }))
    }

    fn menu_handle_pressed(&self) -> M {
        (self.wrap)(HoverCardMessage::Anchor(AnchorEvent::HandlePressed {
            target: self.target.clone(),
        }))
    }

    fn exit(&self) -> M {
        (self.wrap)(HoverCardMessage::Exit {
            target: self.target.clone(),
        })
    }

    fn on_hover_change(
        &self,
        state: &mut HoverState,
        pointer_presence: PointerPresence,
    ) -> Option<M> {
        if pointer_presence == state.pointer_presence {
            return None;
        }
        state.pointer_presence = pointer_presence;
        match (pointer_presence, self.opening) {
            (PointerPresence::PointedAt, NestedMenuOpenTrigger::Hover) => Some(self.enter()),
            (PointerPresence::NotPointedAt, NestedMenuOpenTrigger::Hover) => Some(self.exit()),
            (pointer_presence, NestedMenuOpenTrigger::Press) => {
                Some(self.menu_handle_pointed(pointer_presence))
            }
        }
    }
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
    use corredo::popovers::NestedMenuOpenTrigger;
    use iced::{
        Point, Rectangle, Settings, Size,
        mouse::{Button, Event as MouseEvent},
        widget::{Space, container},
    };
    use iced_test::Simulator;

    use super::{HoverArea, HoverState, hoverable};
    use crate::hover_cards::{
        AnchorEvent, HoverCardMessage, HoverTarget, PointerPresence, WidgetAnchor,
        laid_out_anchors::WidgetLiveness,
    };

    const INSIDE_AREA: Point = Point::new(10.0, 10.0);
    const OUTSIDE_AREA: Point = Point::new(100.0, 100.0);

    fn press_toggled_area<'a>() -> iced::Element<'a, HoverCardMessage> {
        hoverable(
            NestedMenuOpenTrigger::Press,
            container(Space::new()).width(40.0).height(40.0).into(),
            HoverTarget::Workspace(7),
            |message| message,
        )
    }

    fn hovered_area<'a>() -> iced::Element<'a, HoverCardMessage> {
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

    fn area_of(
        target: HoverTarget,
        opening: NestedMenuOpenTrigger,
    ) -> HoverArea<'static, HoverCardMessage> {
        HoverArea {
            content: Space::new().into(),
            target,
            opening,
            wrap: Box::new(|message| message),
        }
    }

    fn area_opening_on(opening: NestedMenuOpenTrigger) -> HoverArea<'static, HoverCardMessage> {
        area_of(HoverTarget::Workspace(7), opening)
    }

    fn fresh_state() -> HoverState {
        HoverState::from(HoverTarget::Workspace(7))
    }

    fn anchor_at(horizontal_position: f32) -> Rectangle {
        Rectangle::new(Point::new(horizontal_position, 0.0), Size::new(40.0, 40.0))
    }

    fn whole_panel() -> Rectangle {
        Rectangle::new(Point::new(0.0, 0.0), Size::new(1000.0, 40.0))
    }

    fn report_of(
        area: &HoverArea<'_, HoverCardMessage>,
        state: &mut HoverState,
        pointer_presence: PointerPresence,
        anchor: Rectangle,
    ) -> Option<HoverCardMessage> {
        area.laid_out_anchor(state, pointer_presence, anchor, &whole_panel())
    }

    #[test]
    fn a_hover_area_stays_silent_until_first_hovered() {
        let area = area_opening_on(NestedMenuOpenTrigger::Hover);
        let mut state = fresh_state();
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::NotPointedAt,
                anchor_at(0.0)
            )
            .is_none()
        );
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::NotPointedAt,
                anchor_at(5.0)
            )
            .is_none()
        );
    }

    #[test]
    fn a_menu_handle_reports_before_any_hover() {
        let area = area_opening_on(NestedMenuOpenTrigger::Press);
        let mut state = fresh_state();
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::NotPointedAt,
                anchor_at(0.0)
            )
            .is_some()
        );
    }

    #[test]
    fn a_hovered_area_reports_each_move_once_even_after_the_pointer_left() {
        let area = area_opening_on(NestedMenuOpenTrigger::Hover);
        let mut state = fresh_state();
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::PointedAt,
                anchor_at(0.0)
            )
            .is_some()
        );
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::PointedAt,
                anchor_at(0.0)
            )
            .is_none()
        );
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::PointedAt,
                anchor_at(5.0)
            )
            .is_some()
        );
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::NotPointedAt,
                anchor_at(5.0)
            )
            .is_none()
        );
        assert!(
            report_of(
                &area,
                &mut state,
                PointerPresence::NotPointedAt,
                anchor_at(9.0)
            )
            .is_some()
        );
    }

    #[test]
    fn a_new_target_in_a_reused_slot_with_identical_bounds_reports_again() {
        let mut state = fresh_state();
        let first = area_opening_on(NestedMenuOpenTrigger::Hover);
        assert!(
            report_of(
                &first,
                &mut state,
                PointerPresence::PointedAt,
                anchor_at(0.0)
            )
            .is_some()
        );
        let second = area_of(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Hover);
        let _replaced = second.exit_of_replaced_target(&mut state);
        assert!(matches!(
            report_of(
                &second,
                &mut state,
                PointerPresence::PointedAt,
                anchor_at(0.0)
            ),
            Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(
                WidgetAnchor {
                    target: HoverTarget::Workspace(8),
                    ..
                }
            )))
        ));
    }

    #[test]
    fn a_replaced_target_exits_only_if_it_was_hovered() {
        let second = area_of(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Hover);
        let mut unhovered = fresh_state();
        assert!(second.exit_of_replaced_target(&mut unhovered).is_none());
        let mut hovered = HoverState {
            pointer_presence: PointerPresence::PointedAt,
            ..fresh_state()
        };
        assert!(matches!(
            second.exit_of_replaced_target(&mut hovered),
            Some(HoverCardMessage::Exit {
                target: HoverTarget::Workspace(7)
            })
        ));
        assert_eq!(hovered.pointer_presence, PointerPresence::NotPointedAt);
        assert!(second.exit_of_replaced_target(&mut hovered).is_none());
    }

    #[test]
    fn a_replaced_menu_handle_stops_being_pointed_at() {
        let second = area_of(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Press);
        let mut hovered = HoverState {
            pointer_presence: PointerPresence::PointedAt,
            ..fresh_state()
        };
        assert!(matches!(
            second.exit_of_replaced_target(&mut hovered),
            Some(HoverCardMessage::Anchor(AnchorEvent::HandlePointed {
                target: HoverTarget::Workspace(7),
                pointing: PointerPresence::NotPointedAt,
                ..
            }))
        ));
    }

    #[test]
    fn dropping_the_state_shows_in_the_reported_presence() {
        let area = area_opening_on(NestedMenuOpenTrigger::Hover);
        let mut state = fresh_state();
        let Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor))) = report_of(
            &area,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(0.0),
        ) else {
            panic!("a hovered area reports");
        };
        assert_eq!(widget_anchor.widget_liveness(), WidgetLiveness::Alive);
        drop(state);
        assert_eq!(widget_anchor.widget_liveness(), WidgetLiveness::Dropped);
    }

    #[test]
    fn a_replaced_target_no_longer_counts_as_the_old_widget() {
        let area = area_opening_on(NestedMenuOpenTrigger::Hover);
        let mut state = fresh_state();
        let Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor))) = report_of(
            &area,
            &mut state,
            PointerPresence::PointedAt,
            anchor_at(0.0),
        ) else {
            panic!("a hovered area reports");
        };
        let replacement = area_of(HoverTarget::Workspace(8), NestedMenuOpenTrigger::Hover);
        let _exit = replacement.exit_of_replaced_target(&mut state);
        assert_eq!(widget_anchor.widget_liveness(), WidgetLiveness::Dropped);
    }

    #[test]
    fn bounds_leaving_the_viewport_report_as_hidden_unless_hovered() {
        let area = area_opening_on(NestedMenuOpenTrigger::Hover);
        let mut state = fresh_state();
        let visibility = |message: Option<HoverCardMessage>| match message {
            Some(HoverCardMessage::Anchor(AnchorEvent::LaidOut(widget_anchor))) => {
                Some(widget_anchor.visible)
            }
            _ => None,
        };
        assert_eq!(
            visibility(report_of(
                &area,
                &mut state,
                PointerPresence::PointedAt,
                anchor_at(0.0)
            )),
            Some(true)
        );
        assert_eq!(
            visibility(report_of(
                &area,
                &mut state,
                PointerPresence::NotPointedAt,
                anchor_at(5000.0)
            )),
            Some(false)
        );
        assert_eq!(
            visibility(report_of(
                &area,
                &mut state,
                PointerPresence::PointedAt,
                anchor_at(6000.0)
            )),
            Some(true)
        );
    }
}
