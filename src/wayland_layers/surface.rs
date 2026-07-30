//! The shared surface vocabulary: which screen edge a surface hugs
//! ([`Position`]) and the per-window identity record ([`Surface`]) held in
//! `Bar`'s bar-surface table. The layer-shell settings that open each surface
//! live with that surface's module ([`bar`](super::bar),
//! [`backdrop_shadow`](super::backdrop_shadow)).

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

/// Which screen edge a surface anchors to. Odin opens both per screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    pub(crate) const ALL: [Position; 2] = [Position::Top, Position::Bottom];

    /// Full-width anchor for a strip hugging this edge; left+right together
    /// make the width compositor-driven. Shared by the bar and its backdrop
    /// caster.
    pub(crate) fn anchor(self) -> Anchor {
        let edge = match self {
            Position::Top => Anchor::Top,
            Position::Bottom => Anchor::Bottom,
        };
        edge | Anchor::Left | Anchor::Right
    }
}

/// The identity of one open bar surface: which output it lives on and which
/// edge it hugs. Rendering picks out that output's workspaces — the whole point
/// of going multi-output. (The off-screen shadow casters carry no such state;
/// they are tracked as bare window ids in `Bar::backdrops`.)
#[derive(Debug, Clone)]
pub struct Surface {
    pub output: String,
    pub position: Position,
}
