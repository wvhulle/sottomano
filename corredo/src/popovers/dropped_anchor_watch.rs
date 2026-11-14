use std::sync::Weak;

use iced::{
    Element, Event, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        mouse, renderer,
        widget::Tree,
    },
    window,
};

#[must_use]
pub fn dropped_anchor_watch<'a, Message: Clone + 'a>(
    presence: Weak<()>,
    on_dropped: Message,
) -> Element<'a, Message> {
    Element::new(DroppedAnchorWatch {
        presence,
        on_dropped,
    })
}

struct DroppedAnchorWatch<Message> {
    presence: Weak<()>,
    on_dropped: Message,
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for DroppedAnchorWatch<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &iced::Renderer, _limits: &Limits) -> Node {
        Node::new(Size::ZERO)
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if matches!(event, Event::Window(window::Event::RedrawRequested(_)))
            && self.presence.strong_count() == 0
        {
            shell.publish(self.on_dropped.clone());
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::disallowed_methods,
    reason = "a redraw event in a test needs a real instant"
)]
mod tests {
    use std::sync::{Arc, Weak};

    use iced::{Settings, Size, time::Instant, window};
    use iced_test::Simulator;

    use super::dropped_anchor_watch;

    fn messages_from_a_redraw(presence: Weak<()>) -> Vec<&'static str> {
        let watch = dropped_anchor_watch(presence, "dropped");
        let mut simulator = Simulator::with_size(Settings::default(), Size::new(10.0, 10.0), watch);
        let _redrawn = simulator.simulate([iced::Event::Window(window::Event::RedrawRequested(
            Instant::now(),
        ))]);
        simulator.into_messages().collect()
    }

    #[test]
    fn a_dropped_anchor_is_noticed_on_the_next_redraw() {
        let anchor = Arc::new(());
        let presence = Arc::downgrade(&anchor);
        assert!(messages_from_a_redraw(presence.clone()).is_empty());
        drop(anchor);
        assert_eq!(messages_from_a_redraw(presence), ["dropped"]);
    }
}
