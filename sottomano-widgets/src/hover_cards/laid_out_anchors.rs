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

use crate::hover_cards::HoverTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardOpening {
    OnHover,
    OnPress,
}

#[derive(Debug, Clone)]
pub struct WidgetAnchor {
    pub target: HoverTarget,
    pub opening: CardOpening,
    pub anchor: Rectangle,
    pub visible: bool,
    pub presence: Weak<()>,
}

impl WidgetAnchor {
    #[must_use]
    pub fn widget_still_exists(&self) -> bool {
        self.presence.strong_count() > 0
    }
}

struct AnchorOnOutput {
    output: String,
    widget_anchor: WidgetAnchor,
    pointed_at: bool,
}

#[derive(Default)]
pub struct LaidOutAnchors {
    anchors: Vec<AnchorOnOutput>,
}

impl LaidOutAnchors {
    pub fn remember(&mut self, output: &str, widget_anchor: WidgetAnchor) {
        let pointed_at = self.is_pointed_at(output, &widget_anchor.target);
        self.anchors.retain(|known| {
            known.output != output || known.widget_anchor.target != widget_anchor.target
        });
        self.anchors.push(AnchorOnOutput {
            output: output.to_owned(),
            widget_anchor,
            pointed_at,
        });
    }

    pub fn set_pointed_at(&mut self, output: &str, target: &HoverTarget, pointed_at: bool) {
        self.anchors
            .iter_mut()
            .filter(|known| known.output == output && known.widget_anchor.target == *target)
            .for_each(|known| known.pointed_at = pointed_at);
    }

    #[must_use]
    pub fn is_pointed_at(&self, output: &str, target: &HoverTarget) -> bool {
        self.anchors.iter().any(|known| {
            known.pointed_at
                && known.output == output
                && known.widget_anchor.target == *target
                && known.widget_anchor.widget_still_exists()
        })
    }

    fn widget_of(&self, output: &str, target: &HoverTarget) -> Option<&WidgetAnchor> {
        self.anchors
            .iter()
            .find(|known| known.output == output && known.widget_anchor.target == *target)
            .map(|known| &known.widget_anchor)
    }

    #[must_use]
    pub fn anchor_of(&self, output: &str, target: &HoverTarget) -> Option<Rectangle> {
        self.widget_of(output, target)
            .map(|widget_anchor| widget_anchor.anchor)
    }

    #[must_use]
    pub fn presence_of(&self, output: &str, target: &HoverTarget) -> Option<Weak<()>> {
        self.widget_of(output, target)
            .map(|widget_anchor| widget_anchor.presence.clone())
    }

    #[must_use]
    pub fn widget_exists(&self, output: &str, target: &HoverTarget) -> bool {
        self.widget_of(output, target)
            .is_some_and(WidgetAnchor::widget_still_exists)
    }

    #[must_use]
    pub fn menu_handles_on(&self, output: &str) -> Vec<Rectangle> {
        self.anchors
            .iter()
            .filter(|known| {
                known.output == output
                    && known.widget_anchor.opening == CardOpening::OnPress
                    && known.widget_anchor.widget_still_exists()
            })
            .map(|known| known.widget_anchor.anchor)
            .collect()
    }

    pub fn forget_output(&mut self, output: &str) {
        self.anchors.retain(|known| known.output != output);
    }
}

pub fn dropped_widget_watch<'a, Message: Clone + 'a>(
    presence: Option<Weak<()>>,
    on_dropped: Message,
) -> Element<'a, Message> {
    Element::new(DroppedWidgetWatch {
        presence,
        on_dropped,
    })
}

struct DroppedWidgetWatch<Message> {
    presence: Option<Weak<()>>,
    on_dropped: Message,
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for DroppedWidgetWatch<Message> {
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
            && self
                .presence
                .as_ref()
                .is_some_and(|presence| presence.strong_count() == 0)
        {
            shell.publish(self.on_dropped.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, LazyLock, Weak};

    use iced::{Point, Rectangle, Settings, Size, window};
    use iced_test::Simulator;

    use super::{CardOpening, LaidOutAnchors, WidgetAnchor, dropped_widget_watch};
    use crate::{clock::Clock, hover_cards::HoverTarget};

    static LIVE_WIDGET: LazyLock<Arc<()>> = LazyLock::new(|| Arc::new(()));

    fn workspace_card() -> HoverTarget {
        HoverTarget::Workspace(7)
    }

    fn anchor_at(horizontal_position: f32) -> Rectangle {
        Rectangle::new(Point::new(horizontal_position, 0.0), Size::new(10.0, 10.0))
    }

    fn handle_at(horizontal_position: f32) -> WidgetAnchor {
        WidgetAnchor {
            target: workspace_card(),
            opening: CardOpening::OnPress,
            anchor: anchor_at(horizontal_position),
            visible: true,
            presence: Arc::downgrade(&LIVE_WIDGET),
        }
    }

    fn messages_from_a_redraw(presence: Option<Weak<()>>) -> Vec<&'static str> {
        let watch = dropped_widget_watch(presence, "dropped");
        let mut simulator = Simulator::with_size(Settings::default(), Size::new(10.0, 10.0), watch);
        let _redrawn = simulator.simulate([iced::Event::Window(window::Event::RedrawRequested(
            Clock::real().instant,
        ))]);
        simulator.into_messages().collect()
    }

    #[test]
    fn a_dropped_widget_is_noticed_on_the_next_redraw() {
        let widget = Arc::new(());
        let presence = Arc::downgrade(&widget);
        assert!(messages_from_a_redraw(Some(presence.clone())).is_empty());
        drop(widget);
        assert_eq!(messages_from_a_redraw(Some(presence)), ["dropped"]);
    }

    #[test]
    fn a_watch_with_nothing_to_watch_stays_silent() {
        assert!(messages_from_a_redraw(None).is_empty());
    }

    #[test]
    fn pointing_at_a_handle_survives_a_new_layout_but_not_a_dropped_widget() {
        let widget = Arc::new(());
        let live_handle = || WidgetAnchor {
            presence: Arc::downgrade(&widget),
            ..handle_at(1.0)
        };
        let mut anchors = LaidOutAnchors::default();
        anchors.remember("DP-1", live_handle());
        anchors.set_pointed_at("DP-1", &workspace_card(), true);
        assert!(anchors.is_pointed_at("DP-1", &workspace_card()));
        assert!(!anchors.is_pointed_at("DP-2", &workspace_card()));
        anchors.remember("DP-1", live_handle());
        assert!(anchors.is_pointed_at("DP-1", &workspace_card()));
        drop(widget);
        assert!(!anchors.is_pointed_at("DP-1", &workspace_card()));
    }

    #[test]
    fn a_dropped_handle_is_no_longer_reachable_below_a_menu() {
        let widget = Arc::new(());
        let mut anchors = LaidOutAnchors::default();
        anchors.remember(
            "DP-1",
            WidgetAnchor {
                presence: Arc::downgrade(&widget),
                ..handle_at(1.0)
            },
        );
        assert!(anchors.widget_exists("DP-1", &workspace_card()));
        assert_eq!(anchors.menu_handles_on("DP-1"), [anchor_at(1.0)]);
        drop(widget);
        assert!(!anchors.widget_exists("DP-1", &workspace_card()));
        assert!(anchors.menu_handles_on("DP-1").is_empty());
    }

    #[test]
    fn a_handle_laid_out_again_replaces_its_anchor_on_that_output_only() {
        let mut anchors = LaidOutAnchors::default();
        anchors.remember("DP-1", handle_at(1.0));
        anchors.remember("DP-2", handle_at(1.0));
        anchors.remember("DP-1", handle_at(2.0));
        assert_eq!(anchors.menu_handles_on("DP-1"), [anchor_at(2.0)]);
        assert_eq!(anchors.menu_handles_on("DP-2"), [anchor_at(1.0)]);
        assert_eq!(
            anchors.anchor_of("DP-1", &workspace_card()),
            Some(anchor_at(2.0))
        );
    }

    #[test]
    fn only_menu_handles_stay_reachable_below_a_menu() {
        let mut anchors = LaidOutAnchors::default();
        anchors.remember(
            "DP-1",
            WidgetAnchor {
                opening: CardOpening::OnHover,
                ..handle_at(1.0)
            },
        );
        assert!(anchors.menu_handles_on("DP-1").is_empty());
    }

    #[test]
    fn forgetting_an_output_removes_only_its_anchors() {
        let mut anchors = LaidOutAnchors::default();
        anchors.remember("DP-1", handle_at(1.0));
        anchors.remember("DP-2", handle_at(1.0));
        anchors.forget_output("DP-2");
        assert!(anchors.menu_handles_on("DP-2").is_empty());
        assert_eq!(anchors.menu_handles_on("DP-1"), [anchor_at(1.0)]);
    }
}
