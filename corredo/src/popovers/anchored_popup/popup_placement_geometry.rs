use iced::{
    Element, Point, Rectangle, Size, Vector,
    advanced::{Layout, layout::Limits, widget::Tree},
    border::Radius,
};

use super::{AnchoredPopup, MorphState, PopupArrival, PopupEdgeClearance, PopupFraming};
use crate::{
    orientation::Edge,
    popovers::{CascadeDirection, CascadeOrientation, PopupGrowth},
    transitions::MotionState,
    width_layout::{ChildView, ChildViewMut},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildFraming {
    LeadingChild,
    WholePopup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PointerPlacement {
    OverPopup,
    OutsidePopup,
}

pub(super) struct PopupFrame {
    pub(super) surface_rows: Vec<Rectangle>,
    pub(super) motion: MotionState,
}

fn over_notched_panel(panel: Layout<'_>, point: Point) -> bool {
    let node = panel.bounds();
    let mut children = panel.children();
    let over_surface_with_gap = children.next().is_some_and(|surface| {
        let surface_bounds = surface.bounds();
        Rectangle::new(
            Point::new(node.x, surface_bounds.y),
            Size::new(node.width, surface_bounds.height),
        )
        .contains(point)
    });
    over_surface_with_gap
        || children.any(|beside_wrapper| {
            beside_wrapper
                .children()
                .next()
                .is_some_and(|beside_panel| over_notched_panel(beside_panel, point))
        })
}

impl<'view, Message, ShownIdentity: 'static> AnchoredPopup<'view, Message, ShownIdentity> {
    pub(super) fn anchor_edge_x(&self) -> f32 {
        self.anchor.x + self.anchor.width
    }

    pub(super) fn dock_line(&self) -> f32 {
        match self.side {
            Edge::Bottom => self.anchor.y + self.anchor.height,
            Edge::Top => self.anchor.y,
            Edge::Right => self.anchor_edge_x(),
            Edge::Left => self.anchor.x,
        }
    }

    pub(super) fn child_elements(&self) -> Vec<&Element<'view, Message>> {
        self.renderings
            .iter()
            .map(|rendering| &rendering.element)
            .collect()
    }

    fn shadow_reach_px(&self) -> f32 {
        match self.edge_clearance {
            PopupEdgeClearance::FlushAgainstEdge => 0.0,
            PopupEdgeClearance::ShadowInsideSurface => {
                let lifted_shadow = (self.frame_over)(Rectangle::default()).lifted_drop_shadow();
                lifted_shadow.blur_radius + lifted_shadow.offset.y.abs()
            }
        }
    }

    pub(super) fn resting(&self, size: Size, surface: Size) -> Rectangle {
        let shadow_reach = self.shadow_reach_px();
        let edge_margin = self.gap_px.max(shadow_reach);
        let centered_on_anchor = |anchor_center: f32, extent: f32, room: f32| {
            (anchor_center - extent / 2.0)
                .min(room - extent - edge_margin)
                .max(edge_margin)
        };
        let horizontal = |extent: f32, room: f32| {
            if self.child_framing() == ChildFraming::LeadingChild {
                (self.anchor.center_x() - self.leading_alignment_px)
                    .min(room - extent - shadow_reach)
                    .max(shadow_reach)
            } else {
                centered_on_anchor(self.anchor.center_x(), extent, room)
            }
        };
        let dock_line = self.dock_line();
        let top_left = match self.side {
            Edge::Bottom => Point::new(
                horizontal(size.width, surface.width),
                dock_line + self.gap_px,
            ),
            Edge::Top => Point::new(
                horizontal(size.width, surface.width),
                dock_line - self.gap_px - size.height,
            ),
            Edge::Right => Point::new(
                dock_line + self.gap_px,
                centered_on_anchor(self.anchor.center_y(), size.height, surface.height),
            ),
            Edge::Left => Point::new(
                dock_line - self.gap_px - size.width,
                centered_on_anchor(self.anchor.center_y(), size.height, surface.height),
            ),
        };
        Rectangle::new(top_left, size)
    }

    pub(super) fn views<'tree>(
        &'tree self,
        tree: &'tree Tree,
        layout: Layout<'tree>,
    ) -> impl DoubleEndedIterator<Item = ChildView<'tree, 'view, Message>> {
        self.renderings
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .map(|((rendering, state), layout)| ChildView {
                widget: &rendering.element,
                state,
                layout,
            })
    }

    pub(super) fn views_mut<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
    ) -> impl DoubleEndedIterator<Item = ChildViewMut<'tree, 'view, Message>> {
        self.renderings
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .map(|((rendering, state), layout)| ChildViewMut {
                widget: &mut rendering.element,
                state,
                layout,
            })
    }

    pub(super) fn cascade_limits(
        &self,
        orientation: CascadeOrientation,
        primary: Size,
        surface: Size,
    ) -> Limits {
        let resting = self.resting(primary, surface);
        let room_beside = match orientation.direction {
            CascadeDirection::Rightward => surface.width - (resting.x + resting.width),
            CascadeDirection::Leftward => resting.x,
        } - self.gap_px;
        let room_along_growth = match orientation.growth {
            PopupGrowth::Downward => surface.height - resting.y,
            PopupGrowth::Upward => resting.y + resting.height,
        } - self.gap_px;
        Limits::new(
            Size::ZERO,
            Size::new(
                primary.width + room_beside.max(0.0),
                room_along_growth.max(0.0),
            ),
        )
    }

    pub(super) fn content_origin(&self, popup: Rectangle, framed_height: f32) -> Point {
        match self.side {
            Edge::Bottom | Edge::Right | Edge::Left => popup.position(),
            Edge::Top => Point::new(popup.x, popup.y + popup.height - framed_height),
        }
    }

    pub(super) fn child_framing(&self) -> ChildFraming {
        if self.renderings.iter().any(|rendering| {
            rendering.arrival == PopupArrival::Incoming
                && matches!(rendering.framing, PopupFraming::LeadingSurface(_))
        }) {
            ChildFraming::LeadingChild
        } else {
            ChildFraming::WholePopup
        }
    }

    pub(super) fn frame_motion(&self, state: &MorphState<ShownIdentity>) -> MotionState {
        let shows_incoming = self
            .renderings
            .iter()
            .any(|rendering| rendering.arrival == PopupArrival::Incoming);
        match state.morph.motion_state() {
            MotionState::Settled if shows_incoming => MotionState::Settled,
            MotionState::Settled | MotionState::Moving => MotionState::Moving,
        }
    }

    pub(super) fn popup_frame(
        &self,
        state: &MorphState<ShownIdentity>,
        layout: Layout<'_>,
    ) -> PopupFrame {
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        PopupFrame {
            surface_rows: state.connector.translated(origin).surface_rows(
                state.morph.current + origin,
                Radius::from(self.corner_radius_px),
            ),
            motion: self.frame_motion(state),
        }
    }

    pub(super) fn pointer_placement(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        point: Point,
    ) -> PointerPlacement {
        let state: &MorphState<ShownIdentity> = tree.state.downcast_ref();
        let origin = Vector::new(layout.bounds().x, layout.bounds().y);
        let over_popup = (state.morph.current + origin).contains(point)
            || self
                .renderings
                .iter()
                .zip(layout.children())
                .filter(|(rendering, _)| {
                    rendering.arrival == PopupArrival::Incoming
                        && matches!(rendering.framing, PopupFraming::LeadingSurface(_))
                })
                .flat_map(|(_, cascade)| cascade.children().skip(1))
                .any(|satellite| over_notched_panel(satellite, point));
        if over_popup {
            PointerPlacement::OverPopup
        } else {
            PointerPlacement::OutsidePopup
        }
    }

    pub(super) fn live_view<'tree>(
        &'tree mut self,
        tree: &'tree mut Tree,
        layout: Layout<'tree>,
    ) -> Option<ChildViewMut<'tree, 'view, Message>> {
        self.renderings
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .find(|((rendering, _), _)| rendering.arrival == PopupArrival::Incoming)
            .map(|((rendering, state), layout)| ChildViewMut {
                widget: &mut rendering.element,
                state,
                layout,
            })
    }
}
