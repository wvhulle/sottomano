use std::sync::Weak;

use iced::Rectangle;

use crate::hover_cards::{HoverTarget, PointerPresence};

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
    pub fn widget_liveness(&self) -> WidgetLiveness {
        if self.presence.strong_count() > 0 {
            WidgetLiveness::Alive
        } else {
            WidgetLiveness::Dropped
        }
    }
}

struct AnchorOnOutput {
    output: String,
    widget_anchor: WidgetAnchor,
    pointing: PointerPresence,
}

#[derive(Default)]
pub struct LaidOutAnchors {
    anchors: Vec<AnchorOnOutput>,
}

impl LaidOutAnchors {
    pub fn remember(&mut self, output: &str, widget_anchor: WidgetAnchor) {
        let pointing = self.handle_pointing(output, &widget_anchor.target);
        self.anchors.retain(|known| {
            known.output != output || known.widget_anchor.target != widget_anchor.target
        });
        self.anchors.push(AnchorOnOutput {
            output: output.to_owned(),
            widget_anchor,
            pointing,
        });
    }

    pub fn set_pointing(&mut self, output: &str, target: &HoverTarget, pointing: PointerPresence) {
        self.anchors
            .iter_mut()
            .filter(|known| known.output == output && known.widget_anchor.target == *target)
            .for_each(|known| known.pointing = pointing);
    }

    #[must_use]
    pub fn handle_pointing(&self, output: &str, target: &HoverTarget) -> PointerPresence {
        if self.anchors.iter().any(|known| {
            known.pointing == PointerPresence::PointedAt
                && known.output == output
                && known.widget_anchor.target == *target
                && known.widget_anchor.widget_liveness() == WidgetLiveness::Alive
        }) {
            PointerPresence::PointedAt
        } else {
            PointerPresence::NotPointedAt
        }
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
    pub fn anchored_widget_liveness(&self, output: &str, target: &HoverTarget) -> WidgetLiveness {
        self.widget_of(output, target)
            .map_or(WidgetLiveness::Dropped, WidgetAnchor::widget_liveness)
    }

    #[must_use]
    pub fn menu_handles_on(&self, output: &str) -> Vec<Rectangle> {
        self.anchors
            .iter()
            .filter(|known| {
                known.output == output
                    && known.widget_anchor.opening == CardOpening::OnPress
                    && known.widget_anchor.widget_liveness() == WidgetLiveness::Alive
            })
            .map(|known| known.widget_anchor.anchor)
            .collect()
    }

    pub fn forget_output(&mut self, output: &str) {
        self.anchors.retain(|known| known.output != output);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetLiveness {
    Alive,
    Dropped,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, LazyLock};

    use iced::{Point, Rectangle, Size};

    use super::{CardOpening, LaidOutAnchors, WidgetAnchor};
    use crate::hover_cards::{HoverTarget, PointerPresence, laid_out_anchors::WidgetLiveness};

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

    #[test]
    fn pointing_at_a_handle_survives_a_new_layout_but_not_a_dropped_widget() {
        let widget = Arc::new(());
        let live_handle = || WidgetAnchor {
            presence: Arc::downgrade(&widget),
            ..handle_at(1.0)
        };
        let mut anchors = LaidOutAnchors::default();
        anchors.remember("DP-1", live_handle());
        anchors.set_pointing("DP-1", &workspace_card(), PointerPresence::PointedAt);
        assert_eq!(
            anchors.handle_pointing("DP-1", &workspace_card()),
            PointerPresence::PointedAt
        );
        assert_eq!(
            anchors.handle_pointing("DP-2", &workspace_card()),
            PointerPresence::NotPointedAt
        );
        anchors.remember("DP-1", live_handle());
        assert_eq!(
            anchors.handle_pointing("DP-1", &workspace_card()),
            PointerPresence::PointedAt
        );
        drop(widget);
        assert_eq!(
            anchors.handle_pointing("DP-1", &workspace_card()),
            PointerPresence::NotPointedAt
        );
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
        assert_eq!(
            anchors.anchored_widget_liveness("DP-1", &workspace_card()),
            WidgetLiveness::Alive
        );
        assert_eq!(anchors.menu_handles_on("DP-1"), [anchor_at(1.0)]);
        drop(widget);
        assert_eq!(
            anchors.anchored_widget_liveness("DP-1", &workspace_card()),
            WidgetLiveness::Dropped
        );
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
