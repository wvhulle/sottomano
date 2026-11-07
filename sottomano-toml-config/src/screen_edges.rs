#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VEdge {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HEdge {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
    Left,
}

impl Position {
    pub const BARS: [Position; 2] = [Position::Top, Position::Bottom];
    pub const ALL: [Position; 3] = [Position::Top, Position::Bottom, Position::Left];
}
