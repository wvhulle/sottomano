use core::{mem::take, slice};

use corredo::popovers::CascadeDirection;
use iced::{
    Border, Element, Event, Length, Point, Rectangle, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Renderer as _, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{Operation, Tree, tree},
    },
    mouse::{Cursor, Interaction},
};

use crate::{
    group_menus::menu_rows::menu_row::{MenuRowLook, menu_row_background, row_text_color},
    hover_cards::PointerPresence,
};

const TRAIL_STEP_PX: f32 = 4.0;
const AIM_SLOPE: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SatelliteReach {
    Open(CascadeDirection),
    Closed,
}

pub(crate) enum RowHoverReaction<Message> {
    Highlights,
    Reveals {
        reveal: Message,
        defer_reveal: Message,
        cancel_reveal: Message,
        satellite_reach: SatelliteReach,
    },
}

pub(crate) struct PointerTrackedRow<'a, Message> {
    body: Element<'a, Message>,
    look: MenuRowLook,
    hover_reaction: RowHoverReaction<Message>,
}

impl<'a, Message: 'a> PointerTrackedRow<'a, Message> {
    pub(crate) fn new(
        body: impl Into<Element<'a, Message>>,
        look: MenuRowLook,
        hover_reaction: RowHoverReaction<Message>,
    ) -> Self {
        Self {
            body: body.into(),
            look,
            hover_reaction,
        }
    }
}

impl<'a, Message: Clone + 'a> From<PointerTrackedRow<'a, Message>> for Element<'a, Message> {
    fn from(row: PointerTrackedRow<'a, Message>) -> Self {
        Element::new(row)
    }
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

#[derive(Default)]
struct RowPointerState {
    pointer_presence: PointerPresence,
    trail: PointerTrail,
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
            PointerPresence::PointedAt
        } else {
            PointerPresence::NotPointedAt
        };
        let state = tree.state.downcast_mut::<RowPointerState>();
        if let Some(position) = cursor.position() {
            state.trail = take(&mut state.trail).recorded(position);
        }
        if state.pointer_presence == pointer_presence {
            return;
        }
        state.pointer_presence = pointer_presence;
        shell.request_redraw();
        let RowHoverReaction::Reveals {
            reveal,
            defer_reveal,
            cancel_reveal,
            satellite_reach,
        } = &self.hover_reaction
        else {
            return;
        };
        let gesture = match (pointer_presence, cursor.position()) {
            (PointerPresence::PointedAt, Some(position)) => {
                match state.trail.aim_at(position, *satellite_reach) {
                    PointerAim::TowardSatellite => defer_reveal,
                    PointerAim::Elsewhere => reveal,
                }
            }
            (PointerPresence::PointedAt, None) => reveal,
            (PointerPresence::NotPointedAt, _) => cancel_reveal,
        };
        shell.publish(gesture.clone());
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
        if let Some(background) =
            menu_row_background(theme, self.look.emphasis, state.pointer_presence)
        {
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
                text_color: row_text_color(self.look.emphasis)(theme),
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
    use corredo::popovers::CascadeDirection;
    use iced::Point;

    use super::{PointerAim, PointerTrail, SatelliteReach};

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
