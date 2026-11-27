use iced::{Element, Rectangle, time::Duration};

use super::{AnchoredPopup, OutsidePress, PopupEdgeClearance, PopupRendering};
use crate::{orientation::Edge, theming::FrameStyle};

const DEFAULT_GAP_PX: f32 = 8.0;
const DEFAULT_MORPH_DURATION: Duration = Duration::from_millis(160);

#[must_use]
pub fn anchored_popup<'view, Message: Clone + 'view>(
    content: impl Into<Element<'view, Message>>,
    anchor: Rectangle,
) -> AnchoredPopup<'view, Message> {
    AnchoredPopup::with_renderings(vec![PopupRendering::incoming(content.into())], anchor)
}

impl<'view, Message: 'view> AnchoredPopup<'view, Message> {
    #[must_use]
    pub fn with_renderings(
        renderings: Vec<PopupRendering<'view, Message>>,
        anchor: Rectangle,
    ) -> Self {
        let frame = FrameStyle::default();
        Self {
            renderings,
            anchor,
            side: Edge::Bottom,
            gap_px: DEFAULT_GAP_PX,
            edge_clearance: PopupEdgeClearance::FlushAgainstEdge,
            leading_alignment_px: 0.0,
            morph_duration: DEFAULT_MORPH_DURATION,
            shown: (),
            outside_press: OutsidePress::PassesThrough,
            frame_over: Box::new(move |_| frame),
            corner_radius_px: frame.corner_radius_px,
        }
    }
}

impl<'view, Message: 'view, ShownIdentity> AnchoredPopup<'view, Message, ShownIdentity> {
    #[must_use]
    pub fn side(self, side: Edge) -> Self {
        Self { side, ..self }
    }

    pub(crate) fn set_anchor(&mut self, anchor: Rectangle) {
        self.anchor = anchor;
    }

    #[must_use]
    pub fn gap(self, gap_px: f32) -> Self {
        Self { gap_px, ..self }
    }

    #[must_use]
    pub fn edge_clearance(self, edge_clearance: PopupEdgeClearance) -> Self {
        Self {
            edge_clearance,
            ..self
        }
    }

    #[must_use]
    pub fn morph_duration(self, morph_duration: Duration) -> Self {
        Self {
            morph_duration,
            ..self
        }
    }

    #[must_use]
    pub fn frame(self, frame: FrameStyle) -> Self {
        Self {
            frame_over: Box::new(move |_| frame),
            corner_radius_px: frame.corner_radius_px,
            ..self
        }
    }

    #[must_use]
    pub fn frame_over(self, frame_over: impl Fn(Rectangle) -> FrameStyle + 'view) -> Self {
        let corner_radius_px = frame_over(Rectangle::default()).corner_radius_px;
        Self {
            frame_over: Box::new(frame_over),
            corner_radius_px,
            ..self
        }
    }

    #[must_use]
    pub fn dismissed_by(self, dismissal: Message) -> Self {
        Self {
            outside_press: OutsidePress::Dismisses {
                dismissal,
                reachable_below: Vec::new(),
            },
            ..self
        }
    }

    #[must_use]
    pub fn outside_press(self, outside_press: OutsidePress<Message>) -> Self {
        Self {
            outside_press,
            ..self
        }
    }

    #[must_use]
    pub fn leading_alignment(self, leading_alignment_px: f32) -> Self {
        Self {
            leading_alignment_px,
            ..self
        }
    }

    #[must_use]
    pub fn identified_by<OtherIdentity>(
        self,
        shown: OtherIdentity,
    ) -> AnchoredPopup<'view, Message, OtherIdentity> {
        AnchoredPopup {
            renderings: self.renderings,
            anchor: self.anchor,
            side: self.side,
            gap_px: self.gap_px,
            edge_clearance: self.edge_clearance,
            leading_alignment_px: self.leading_alignment_px,
            morph_duration: self.morph_duration,
            shown,
            outside_press: self.outside_press,
            frame_over: self.frame_over,
            corner_radius_px: self.corner_radius_px,
        }
    }
}

impl<'view, Message: Clone + 'view, ShownIdentity: Clone + PartialEq + 'static>
    From<AnchoredPopup<'view, Message, ShownIdentity>> for Element<'view, Message>
{
    fn from(popup: AnchoredPopup<'view, Message, ShownIdentity>) -> Self {
        Element::new(popup)
    }
}
