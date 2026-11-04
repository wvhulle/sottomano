use iced::Rectangle;

use crate::hover_cards::HoverTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardOpening {
    OnHover,
    OnPress,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetAnchor {
    pub target: HoverTarget,
    pub opening: CardOpening,
    pub anchor: Rectangle,
    pub visible: bool,
}

struct AnchorOnOutput {
    output: String,
    widget_anchor: WidgetAnchor,
}

#[derive(Default)]
pub struct LaidOutAnchors {
    anchors: Vec<AnchorOnOutput>,
}

impl LaidOutAnchors {
    pub fn remember(&mut self, output: &str, widget_anchor: WidgetAnchor) {
        self.anchors.retain(|known| {
            known.output != output || known.widget_anchor.target != widget_anchor.target
        });
        self.anchors.push(AnchorOnOutput {
            output: output.to_owned(),
            widget_anchor,
        });
    }

    #[must_use]
    pub fn anchor_of(&self, output: &str, target: &HoverTarget) -> Option<Rectangle> {
        self.anchors
            .iter()
            .find(|known| known.output == output && known.widget_anchor.target == *target)
            .map(|known| known.widget_anchor.anchor)
    }

    #[must_use]
    pub fn menu_handles_on(&self, output: &str) -> Vec<Rectangle> {
        self.anchors
            .iter()
            .filter(|known| {
                known.output == output && known.widget_anchor.opening == CardOpening::OnPress
            })
            .map(|known| known.widget_anchor.anchor)
            .collect()
    }

    pub fn forget_output(&mut self, output: &str) {
        self.anchors.retain(|known| known.output != output);
    }
}

#[cfg(test)]
mod tests {
    use iced::{Point, Rectangle, Size};

    use super::{CardOpening, LaidOutAnchors, WidgetAnchor};
    use crate::hover_cards::HoverTarget;

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
        }
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
