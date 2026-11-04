use std::{mem::replace, slice::from_ref, sync::Arc};

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
        CardOpening, HoverAnnouncement, HoverCardMessage, HoverTarget, PressHints, WidgetAnchor,
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

struct HoverArea<'a, M> {
    content: Element<'a, M>,
    target: HoverTarget,
    announcement: HoverAnnouncement,
    opening: CardOpening,
    wrap: Box<dyn Fn(HoverCardMessage) -> M + 'a>,
}

struct HoverState {
    target: HoverTarget,
    hovered: bool,
    announced: HoverAnnouncement,
    reported_anchor: Option<Rectangle>,
    reported_visible: bool,
    presence: Arc<()>,
}

impl HoverState {
    fn of(target: HoverTarget) -> Self {
        Self {
            target,
            hovered: false,
            announced: HoverAnnouncement::default(),
            reported_anchor: None,
            reported_visible: false,
            presence: Arc::new(()),
        }
    }
}

impl<M> HoverArea<'_, M> {
    fn exit_of_replaced_target(&self, state: &mut HoverState) -> Option<M> {
        if state.target == self.target {
            return None;
        }
        let replaced = replace(state, HoverState::of(self.target.clone()));
        replaced.hovered.then(|| {
            (self.wrap)(match self.opening {
                CardOpening::OnHover => HoverCardMessage::Exit {
                    target: replaced.target,
                },
                CardOpening::OnPress => HoverCardMessage::MenuHandlePointedAnchor {
                    target: replaced.target,
                    pointed_at: false,
                    announcement: replaced.announced,
                },
            })
        })
    }

    fn laid_out_anchor(
        &self,
        state: &mut HoverState,
        hovered: bool,
        anchor: Rectangle,
        viewport: &Rectangle,
    ) -> Option<M> {
        let reports =
            self.opening == CardOpening::OnPress || hovered || state.reported_anchor.is_some();
        let visible = hovered || anchor.intersection(viewport).is_some();
        if !reports || (state.reported_anchor == Some(anchor) && state.reported_visible == visible)
        {
            return None;
        }
        state.reported_anchor = Some(anchor);
        state.reported_visible = visible;
        Some((self.wrap)(HoverCardMessage::AnchorLaidOut(WidgetAnchor {
            target: self.target.clone(),
            opening: self.opening,
            anchor,
            visible,
            presence: Arc::downgrade(&state.presence),
        })))
    }

    fn enter(&self, state: &mut HoverState) -> M {
        state.announced = self.announcement.clone();
        (self.wrap)(HoverCardMessage::EnterAnchor {
            target: self.target.clone(),
            announcement: self.announcement.clone(),
        })
    }

    fn menu_handle_pointed(&self, pointed_at: bool) -> M {
        (self.wrap)(HoverCardMessage::MenuHandlePointedAnchor {
            target: self.target.clone(),
            pointed_at,
            announcement: self.announcement.clone(),
        })
    }

    fn menu_handle_pressed(&self) -> M {
        (self.wrap)(HoverCardMessage::MenuHandlePressedAnchor {
            target: self.target.clone(),
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

    fn on_hover_change(&self, state: &mut HoverState, hovered: bool) -> Option<M> {
        if hovered == state.hovered {
            return None;
        }
        state.hovered = hovered;
        match (hovered, self.opening) {
            (true, CardOpening::OnHover) => Some(self.enter(state)),
            (true, CardOpening::OnPress) => Some(self.menu_handle_pointed(true)),
            (false, CardOpening::OnHover) => Some(self.exit()),
            (false, CardOpening::OnPress) => Some(self.menu_handle_pointed(false)),
        }
    }
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for HoverArea<'_, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<HoverState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(HoverState::of(self.target.clone()))
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
        let hovered = cursor.is_over(anchor);
        if let Some(laid_out) = self.laid_out_anchor(state, hovered, anchor, viewport) {
            shell.publish(laid_out);
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
            shell.publish(self.menu_handle_pressed());
            shell.capture_event();
            return;
        }
        let published = self
            .on_hover_change(state, hovered)
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
        Point, Rectangle, Settings, Size,
        mouse::{Button, Event as MouseEvent},
        widget::{Space, container},
    };
    use iced_test::Simulator;

    use super::{HoverArea, HoverState, announcing_hoverable, press_toggled_hoverable};
    use crate::hover_cards::{
        CardOpening, HoverAnnouncement, HoverCardMessage, HoverTarget, WidgetAnchor,
    };

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

    #[test]
    fn hovering_a_menu_handle_reports_it_without_entering() {
        let messages = messages_from_pointing_inside(press_toggled_area(), false);
        assert!(
            matches!(
                messages.as_slice(),
                [
                    HoverCardMessage::AnchorLaidOut(..),
                    HoverCardMessage::MenuHandlePointedAnchor {
                        pointed_at: true,
                        ..
                    }
                ]
            ),
            "{messages:?}"
        );
    }

    #[test]
    fn pressing_a_menu_handle_reports_the_press_after_its_layout() {
        let messages = messages_from_pointing_inside(press_toggled_area(), true);
        assert!(
            matches!(
                messages.as_slice(),
                [
                    HoverCardMessage::AnchorLaidOut(..),
                    HoverCardMessage::MenuHandlePointedAnchor { .. },
                    HoverCardMessage::MenuHandlePressedAnchor { .. }
                ]
            ),
            "{messages:?}"
        );
    }

    #[test]
    fn hovering_a_hover_area_opens_its_card() {
        let messages = messages_from_pointing_inside(hovered_area(), false);
        assert!(
            matches!(
                messages.as_slice(),
                [
                    HoverCardMessage::AnchorLaidOut(..),
                    HoverCardMessage::EnterAnchor { .. }
                ]
            ),
            "{messages:?}"
        );
    }

    fn area_of(target: HoverTarget, opening: CardOpening) -> HoverArea<'static, HoverCardMessage> {
        HoverArea {
            content: Space::new().into(),
            target,
            announcement: HoverAnnouncement::default(),
            opening,
            wrap: Box::new(|message| message),
        }
    }

    fn area_opening_on(opening: CardOpening) -> HoverArea<'static, HoverCardMessage> {
        area_of(HoverTarget::Workspace(7), opening)
    }

    fn fresh_state() -> HoverState {
        HoverState::of(HoverTarget::Workspace(7))
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
        hovered: bool,
        anchor: Rectangle,
    ) -> Option<HoverCardMessage> {
        area.laid_out_anchor(state, hovered, anchor, &whole_panel())
    }

    #[test]
    fn a_hover_area_stays_silent_until_first_hovered() {
        let area = area_opening_on(CardOpening::OnHover);
        let mut state = fresh_state();
        assert!(report_of(&area, &mut state, false, anchor_at(0.0)).is_none());
        assert!(report_of(&area, &mut state, false, anchor_at(5.0)).is_none());
    }

    #[test]
    fn a_menu_handle_reports_before_any_hover() {
        let area = area_opening_on(CardOpening::OnPress);
        let mut state = fresh_state();
        assert!(report_of(&area, &mut state, false, anchor_at(0.0)).is_some());
    }

    #[test]
    fn a_hovered_area_reports_each_move_once_even_after_the_pointer_left() {
        let area = area_opening_on(CardOpening::OnHover);
        let mut state = fresh_state();
        assert!(report_of(&area, &mut state, true, anchor_at(0.0)).is_some());
        assert!(report_of(&area, &mut state, true, anchor_at(0.0)).is_none());
        assert!(report_of(&area, &mut state, true, anchor_at(5.0)).is_some());
        assert!(report_of(&area, &mut state, false, anchor_at(5.0)).is_none());
        assert!(report_of(&area, &mut state, false, anchor_at(9.0)).is_some());
    }

    #[test]
    fn a_new_target_in_a_reused_slot_with_identical_bounds_reports_again() {
        let mut state = fresh_state();
        let first = area_opening_on(CardOpening::OnHover);
        assert!(report_of(&first, &mut state, true, anchor_at(0.0)).is_some());
        let second = area_of(HoverTarget::Workspace(8), CardOpening::OnHover);
        let _replaced = second.exit_of_replaced_target(&mut state);
        assert!(matches!(
            report_of(&second, &mut state, true, anchor_at(0.0)),
            Some(HoverCardMessage::AnchorLaidOut(WidgetAnchor {
                target: HoverTarget::Workspace(8),
                ..
            }))
        ));
    }

    #[test]
    fn a_replaced_target_exits_only_if_it_was_hovered() {
        let second = area_of(HoverTarget::Workspace(8), CardOpening::OnHover);
        let mut unhovered = fresh_state();
        assert!(second.exit_of_replaced_target(&mut unhovered).is_none());
        let mut hovered = HoverState {
            hovered: true,
            ..fresh_state()
        };
        assert!(matches!(
            second.exit_of_replaced_target(&mut hovered),
            Some(HoverCardMessage::Exit {
                target: HoverTarget::Workspace(7)
            })
        ));
        assert!(!hovered.hovered);
        assert!(second.exit_of_replaced_target(&mut hovered).is_none());
    }

    #[test]
    fn a_replaced_menu_handle_stops_being_pointed_at() {
        let second = area_of(HoverTarget::Workspace(8), CardOpening::OnPress);
        let mut hovered = HoverState {
            hovered: true,
            ..fresh_state()
        };
        assert!(matches!(
            second.exit_of_replaced_target(&mut hovered),
            Some(HoverCardMessage::MenuHandlePointedAnchor {
                target: HoverTarget::Workspace(7),
                pointed_at: false,
                ..
            })
        ));
    }

    #[test]
    fn dropping_the_state_shows_in_the_reported_presence() {
        let area = area_opening_on(CardOpening::OnHover);
        let mut state = fresh_state();
        let Some(HoverCardMessage::AnchorLaidOut(widget_anchor)) =
            report_of(&area, &mut state, true, anchor_at(0.0))
        else {
            panic!("a hovered area reports");
        };
        assert!(widget_anchor.widget_still_exists());
        drop(state);
        assert!(!widget_anchor.widget_still_exists());
    }

    #[test]
    fn a_replaced_target_no_longer_counts_as_the_old_widget() {
        let area = area_opening_on(CardOpening::OnHover);
        let mut state = fresh_state();
        let Some(HoverCardMessage::AnchorLaidOut(widget_anchor)) =
            report_of(&area, &mut state, true, anchor_at(0.0))
        else {
            panic!("a hovered area reports");
        };
        let replacement = area_of(HoverTarget::Workspace(8), CardOpening::OnHover);
        let _exit = replacement.exit_of_replaced_target(&mut state);
        assert!(!widget_anchor.widget_still_exists());
    }

    #[test]
    fn bounds_leaving_the_viewport_report_as_hidden_unless_hovered() {
        let area = area_opening_on(CardOpening::OnHover);
        let mut state = fresh_state();
        let visibility = |message: Option<HoverCardMessage>| match message {
            Some(HoverCardMessage::AnchorLaidOut(widget_anchor)) => Some(widget_anchor.visible),
            _ => None,
        };
        assert_eq!(
            visibility(report_of(&area, &mut state, true, anchor_at(0.0))),
            Some(true)
        );
        assert_eq!(
            visibility(report_of(&area, &mut state, false, anchor_at(5000.0))),
            Some(false)
        );
        assert_eq!(
            visibility(report_of(&area, &mut state, true, anchor_at(6000.0))),
            Some(true)
        );
    }
}
