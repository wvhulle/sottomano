use iced::{
    Element, Size, Vector,
    widget::{float, pin},
};

use crate::widgets::pointer_drag::ActiveDrag;

#[derive(Debug, Clone, Copy)]
pub enum GhostPlacement {
    UnderCursor,
    BesideCursor { gap: f32 },
}

impl GhostPlacement {
    fn offset_from_cursor(self, ghost: Size) -> Vector {
        match self {
            GhostPlacement::UnderCursor => Vector::new(-ghost.width / 2.0, -ghost.height / 2.0),
            GhostPlacement::BesideCursor { gap } => Vector::new(gap, -ghost.height / 2.0),
        }
    }
}

pub fn drag_ghost<'a, Message: 'a>(
    card: Element<'a, Message>,
    drag: &ActiveDrag,
    placement: GhostPlacement,
    scale: f32,
) -> Element<'a, Message> {
    let lifted = float(card)
        .scale(scale)
        .translate(move |bounds, _viewport| placement.offset_from_cursor(bounds.size()));
    pin(lifted).x(drag.cursor.x).y(drag.cursor.y).into()
}
