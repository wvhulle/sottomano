use iced::{
    Element, Size, Vector,
    widget::{float, pin},
};

use crate::drag_reorder::ActiveDrag;

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

#[must_use]
pub fn drag_ghost<'view, Message: 'view, SlotId>(
    card: Element<'view, Message>,
    drag: &ActiveDrag<SlotId>,
    placement: GhostPlacement,
    scale: f32,
) -> Element<'view, Message> {
    let lifted = float(card)
        .scale(scale)
        .translate(move |bounds, _viewport| placement.offset_from_cursor(bounds.size()));
    pin(lifted).x(drag.cursor.x).y(drag.cursor.y).into()
}
