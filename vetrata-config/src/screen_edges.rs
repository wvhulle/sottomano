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
