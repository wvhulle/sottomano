use iced_layershell::reexport::Anchor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Margin {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Margin {
    pub fn edges(self) -> (i32, i32, i32, i32) {
        (self.top, self.right, self.bottom, self.left)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl SurfaceSize {
    pub fn dimensions(self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    pub(crate) const ALL: [Position; 2] = [Position::Top, Position::Bottom];

    pub(crate) fn anchor(self) -> Anchor {
        let edge = match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
        };
        edge | Anchor::Left | Anchor::Right
    }
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub position: Position,
}
