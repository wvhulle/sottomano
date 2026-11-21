use iced::{Rectangle, mouse::Cursor};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum PointerHover {
    #[default]
    Outside,
    Over,
}

pub(crate) fn hover_over_bounds(cursor: Cursor, bounds: Rectangle) -> PointerHover {
    if cursor.is_over(bounds) {
        PointerHover::Over
    } else {
        PointerHover::Outside
    }
}
