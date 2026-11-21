use core::{mem::take, slice};
use std::time::Instant;

use iced::{
    Border, Element, Event, Length, Point, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::{Cursor, Interaction},
    window,
};

use super::{MenuRowLook, RowHoverReaction, SatelliteReach};
use crate::popovers::CascadeDirection;

const TRAIL_STEP_PX: f32 = 4.0;
const AIM_SLOPE: f32 = 2.0;

pub(super) struct PointerTrackedRow<'view, Message> {
    pub(super) body: Element<'view, Message>,
    pub(super) look: MenuRowLook,
    pub(super) hover_reaction: RowHoverReaction<Message>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerAim {
    TowardSatellite,
    Elsewhere,
}

#[derive(Default)]
enum PointerTrail {
    #[default]
    Unseen,
    Arrived(Point),
    Moving {
        earlier: Point,
        recent: Point,
    },
}

impl PointerTrail {
    fn recorded(self, position: Point) -> Self {
        match self {
            PointerTrail::Unseen => PointerTrail::Arrived(position),
            PointerTrail::Arrived(recent) | PointerTrail::Moving { recent, .. }
                if recent.distance(position) >= TRAIL_STEP_PX =>
            {
                PointerTrail::Moving {
                    earlier: recent,
                    recent: position,
                }
            }
            unmoved => unmoved,
        }
    }

    fn aim_at(&self, position: Point, satellite_reach: SatelliteReach) -> PointerAim {
        let (PointerTrail::Moving { earlier, .. }, SatelliteReach::Open(direction)) =
            (self, satellite_reach)
        else {
            return PointerAim::Elsewhere;
        };
        let toward_satellite = match direction {
            CascadeDirection::Rightward => position.x - earlier.x,
            CascadeDirection::Leftward => earlier.x - position.x,
        };
        if toward_satellite > 0.0 && toward_satellite * AIM_SLOPE >= (position.y - earlier.y).abs()
        {
            PointerAim::TowardSatellite
        } else {
            PointerAim::Elsewhere
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum RowPointerPresence {
    PointedAt,
    #[default]
    NotPointedAt,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PendingReveal {
    #[default]
    Idle,
    Armed,
    Until(Instant),
}

#[derive(Default)]
struct RowPointerState {
    pointer_presence: RowPointerPresence,
    trail: PointerTrail,
    pending_reveal: PendingReveal,
}

impl<Message: Clone> Widget<Message, Theme, iced::Renderer> for PointerTrackedRow<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<RowPointerState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(RowPointerState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.body)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(slice::from_ref(&self.body));
    }

    fn size(&self) -> Size<Length> {
        self.body.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &Limits) -> Node {
        let body_node = self
            .body
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        Node::with_children(body_node.size(), vec![body_node])
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
        self.body.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let pointer_presence = if cursor.is_over(layout.bounds()) {
            RowPointerPresence::PointedAt
        } else {
            RowPointerPresence::NotPointedAt
        };
        let state = tree.state.downcast_mut::<RowPointerState>();
        if let Some(position) = cursor.position() {
            state.trail = take(&mut state.trail).recorded(position);
        }
        let RowHoverReaction::Reveals {
            reveal,
            satellite_reach,
            reveal_delay,
        } = &self.hover_reaction
        else {
            if state.pointer_presence != pointer_presence {
                state.pointer_presence = pointer_presence;
                shell.request_redraw();
            }
            return;
        };
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            match state.pending_reveal {
                PendingReveal::Idle => {}
                PendingReveal::Armed => {
                    let deadline = *now + *reveal_delay;
                    state.pending_reveal = PendingReveal::Until(deadline);
                    shell.request_redraw_at(deadline);
                }
                PendingReveal::Until(deadline) if *now >= deadline => {
                    state.pending_reveal = PendingReveal::Idle;
                    shell.publish(reveal.clone());
                }
                PendingReveal::Until(deadline) => shell.request_redraw_at(deadline),
            }
        }
        if state.pointer_presence == pointer_presence {
            return;
        }
        state.pointer_presence = pointer_presence;
        shell.request_redraw();
        state.pending_reveal = PendingReveal::Idle;
        if pointer_presence == RowPointerPresence::NotPointedAt {
            return;
        }
        let aim = match cursor.position() {
            Some(position) => state.trail.aim_at(position, *satellite_reach),
            None => PointerAim::Elsewhere,
        };
        match aim {
            PointerAim::TowardSatellite => state.pending_reveal = PendingReveal::Armed,
            PointerAim::Elsewhere => shell.publish(reveal.clone()),
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<RowPointerState>();
        if let Some(background) = self.look.emphasis.background(theme, state.pointer_presence) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    border: Border {
                        radius: self.look.frame_corners,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                background,
            );
        }
        self.body.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style {
                text_color: self.look.emphasis.text_color()(theme),
            },
            layout.children().next().unwrap(),
            cursor,
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
    ) -> Interaction {
        self.body.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.body.as_widget_mut().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
    }

    fn overlay<'overlay>(
        &'overlay mut self,
        tree: &'overlay mut Tree,
        layout: Layout<'overlay>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'overlay, Message, Theme, iced::Renderer>> {
        self.body.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use iced::Point;

    use super::{PointerAim, PointerTrail, SatelliteReach};
    use crate::popovers::CascadeDirection;

    const ENTERED_LEFT: Point = Point::new(20.0, 40.0);
    const ENTERED_RIGHT: Point = Point::new(80.0, 40.0);

    fn aim_from_above(entered_at: Point, satellite_reach: SatelliteReach) -> PointerAim {
        PointerTrail::default()
            .recorded(Point::new(50.0, 10.0))
            .recorded(entered_at)
            .aim_at(entered_at, satellite_reach)
    }

    #[test]
    fn moving_away_from_the_satellite_does_not_aim_at_it() {
        assert_eq!(
            aim_from_above(
                ENTERED_LEFT,
                SatelliteReach::Open(CascadeDirection::Rightward)
            ),
            PointerAim::Elsewhere
        );
    }

    #[test]
    fn without_an_open_satellite_nothing_is_aimed_at() {
        assert_eq!(
            aim_from_above(ENTERED_RIGHT, SatelliteReach::Closed),
            PointerAim::Elsewhere
        );
    }

    #[test]
    fn a_leftward_satellite_is_aimed_at_by_moving_left() {
        assert_eq!(
            aim_from_above(
                ENTERED_LEFT,
                SatelliteReach::Open(CascadeDirection::Leftward)
            ),
            PointerAim::TowardSatellite
        );
    }
}
