use iced::{
    Element, Event, Length, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    mouse::Cursor,
    time::Duration,
    widget::container,
};

use crate::{
    orientation::Edge,
    popovers::{AnchoredPopup, anchored_popup},
    theming::FrameStyle,
};

const TRIGGER_CHILD: usize = 0;
const POPUP_CHILD: usize = 1;
const DEFAULT_CONTENT_PADDING_PX: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupVisibility {
    Open,
    #[default]
    Closed,
}

pub struct PopupTrigger<'a, Message> {
    trigger: Element<'a, Message>,
    popup: AnchoredPopup<'a, Message>,
    visibility: PopupVisibility,
}

pub fn popup_trigger<'a, Message: Clone + 'a>(
    trigger: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    visibility: PopupVisibility,
) -> PopupTrigger<'a, Message> {
    PopupTrigger {
        trigger: trigger.into(),
        popup: anchored_popup(
            container(content).padding(DEFAULT_CONTENT_PADDING_PX),
            Rectangle::default(),
        ),
        visibility,
    }
}

impl<'a, Message: Clone + 'a> PopupTrigger<'a, Message> {
    #[must_use]
    pub fn side(self, side: Edge) -> Self {
        Self {
            popup: self.popup.side(side),
            ..self
        }
    }

    #[must_use]
    pub fn gap(self, gap_px: f32) -> Self {
        Self {
            popup: self.popup.gap(gap_px),
            ..self
        }
    }

    #[must_use]
    pub fn frame(self, frame: FrameStyle) -> Self {
        Self {
            popup: self.popup.frame(frame),
            ..self
        }
    }

    #[must_use]
    pub fn morph_duration(self, morph_duration: Duration) -> Self {
        Self {
            popup: self.popup.morph_duration(morph_duration),
            ..self
        }
    }

    #[must_use]
    pub fn on_dismiss(self, dismissal: Message) -> Self {
        Self {
            popup: self.popup.dismissed_by(dismissal),
            ..self
        }
    }
}

impl<'a, Message: Clone + 'a> From<PopupTrigger<'a, Message>> for Element<'a, Message> {
    fn from(trigger: PopupTrigger<'a, Message>) -> Self {
        Element::new(trigger)
    }
}

impl<'a, Message: Clone + 'a> Widget<Message, Theme, iced::Renderer> for PopupTrigger<'a, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![
            Tree::new(&self.trigger),
            Tree::new(&self.popup as &dyn Widget<Message, Theme, iced::Renderer>),
        ]
    }

    fn diff(&self, tree: &mut Tree) {
        if tree.children.len() <= POPUP_CHILD {
            tree.children = Widget::<Message, Theme, iced::Renderer>::children(self);
            return;
        }
        tree.children[TRIGGER_CHILD].diff(&self.trigger);
        let popup: &dyn Widget<Message, Theme, iced::Renderer> = &self.popup;
        if self.visibility == PopupVisibility::Open {
            tree.children[POPUP_CHILD].diff(popup);
        } else {
            tree.children[POPUP_CHILD] = Tree::new(popup);
        }
    }

    fn size(&self) -> Size<Length> {
        self.trigger.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        self.trigger
            .as_widget_mut()
            .layout(&mut tree.children[TRIGGER_CHILD], renderer, limits)
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
        self.trigger.as_widget().draw(
            &tree.children[TRIGGER_CHILD],
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
        self.trigger.as_widget_mut().operate(
            &mut tree.children[TRIGGER_CHILD],
            layout,
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
        self.trigger.as_widget_mut().update(
            &mut tree.children[TRIGGER_CHILD],
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
        self.trigger.as_widget().mouse_interaction(
            &tree.children[TRIGGER_CHILD],
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
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        let (trigger_tree, popup_tree) = tree.children.split_at_mut(POPUP_CHILD);
        if self.visibility == PopupVisibility::Closed {
            return self.trigger.as_widget_mut().overlay(
                &mut trigger_tree[TRIGGER_CHILD],
                layout,
                renderer,
                viewport,
                translation,
            );
        }
        self.popup.set_anchor(layout.bounds() + translation);
        Some(overlay::Element::new(Box::new(PopupOverlay {
            popup: &mut self.popup,
            tree: &mut popup_tree[0],
        })))
    }
}

struct PopupOverlay<'b, 'a, Message> {
    popup: &'b mut AnchoredPopup<'a, Message>,
    tree: &'b mut Tree,
}

impl<Message: Clone> overlay::Overlay<Message, Theme, iced::Renderer>
    for PopupOverlay<'_, '_, Message>
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> Node {
        Widget::<Message, Theme, iced::Renderer>::layout(
            self.popup,
            self.tree,
            renderer,
            &Limits::new(Size::ZERO, bounds),
        )
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
    ) {
        Widget::<Message, Theme, iced::Renderer>::draw(
            &*self.popup,
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        Widget::<Message, Theme, iced::Renderer>::operate(
            self.popup, self.tree, layout, renderer, operation,
        );
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
        let viewport = layout.bounds();
        Widget::<Message, Theme, iced::Renderer>::update(
            self.popup, self.tree, event, layout, cursor, renderer, clipboard, shell, &viewport,
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        Widget::<Message, Theme, iced::Renderer>::mouse_interaction(
            &*self.popup,
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use iced::{
        Point, Settings,
        alignment::{Horizontal, Vertical},
        mouse::{Button, Event as MouseEvent},
        time::Instant,
        widget::{button, container, text},
        window,
    };
    use iced_test::Simulator;

    use super::*;
    use crate::orientation::Edge;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum PopupMessage {
        Toggled,
        Dismissed,
        InsidePressed,
    }

    const WINDOW_SIZE: Size = Size::new(600.0, 400.0);
    const FIRST_LABEL: &str = "open popup";
    const INSIDE_LABEL: &str = "inside the popup";

    fn window_with_popup<'a>(visibility: PopupVisibility, side: Edge) -> Element<'a, PopupMessage> {
        let trigger = button(text(FIRST_LABEL)).on_press(PopupMessage::Toggled);
        let content = button(text(INSIDE_LABEL)).on_press(PopupMessage::InsidePressed);
        container(
            popup_trigger(trigger, content, visibility)
                .side(side)
                .on_dismiss(PopupMessage::Dismissed),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }

    fn settled_simulator(popup: Element<'_, PopupMessage>) -> Simulator<'_, PopupMessage> {
        let mut simulator = Simulator::with_size(Settings::default(), WINDOW_SIZE, popup);
        let started = Instant::now();
        let _settled = simulator.simulate([
            Event::Window(window::Event::RedrawRequested(started)),
            Event::Window(window::Event::RedrawRequested(
                started + Duration::from_secs(1),
            )),
        ]);
        simulator
    }

    fn bounds_of(simulator: &mut Simulator<'_, PopupMessage>, shown: &str) -> Rectangle {
        simulator
            .find(shown)
            .expect("the text is on screen")
            .visible_bounds()
            .expect("the text is visible")
    }

    fn press_at(simulator: &mut Simulator<'_, PopupMessage>, pointer: Point) {
        simulator.point_at(pointer);
        let _pressed = simulator.simulate([
            Event::Mouse(MouseEvent::CursorMoved { position: pointer }),
            Event::Mouse(MouseEvent::ButtonPressed(Button::Left)),
            Event::Mouse(MouseEvent::ButtonReleased(Button::Left)),
        ]);
    }

    #[test]
    fn a_closed_popup_shows_only_the_trigger() {
        let mut simulator =
            settled_simulator(window_with_popup(PopupVisibility::Closed, Edge::Bottom));
        assert!(simulator.find(FIRST_LABEL).is_ok());
        assert!(simulator.find(INSIDE_LABEL).is_err());
    }

    #[test]
    fn each_side_places_the_popup_next_to_the_trigger() {
        let trigger_and_popup = |side| {
            let mut simulator = settled_simulator(window_with_popup(PopupVisibility::Open, side));
            (
                bounds_of(&mut simulator, FIRST_LABEL),
                bounds_of(&mut simulator, INSIDE_LABEL),
            )
        };
        let (trigger, below) = trigger_and_popup(Edge::Bottom);
        assert!(below.y > trigger.y + trigger.height);
        let (trigger, above) = trigger_and_popup(Edge::Top);
        assert!(above.y + above.height < trigger.y);
        let (trigger, right) = trigger_and_popup(Edge::Right);
        assert!(right.x > trigger.x + trigger.width);
        let (trigger, left) = trigger_and_popup(Edge::Left);
        assert!(left.x + left.width < trigger.x);
    }

    #[test]
    fn a_press_outside_dismisses_and_a_press_inside_reaches_the_content() {
        let mut simulator =
            settled_simulator(window_with_popup(PopupVisibility::Open, Edge::Bottom));
        let inside = bounds_of(&mut simulator, INSIDE_LABEL).center();
        press_at(&mut simulator, inside);
        press_at(&mut simulator, Point::new(5.0, 5.0));
        assert_eq!(
            simulator.into_messages().collect::<Vec<_>>(),
            [PopupMessage::InsidePressed, PopupMessage::Dismissed]
        );
    }

    #[test]
    fn the_popup_renders_in_a_plain_window() {
        let mut simulator =
            settled_simulator(window_with_popup(PopupVisibility::Open, Edge::Bottom));
        let _rendered = simulator.snapshot(&Theme::Dark).expect("the frame renders");
    }
}
