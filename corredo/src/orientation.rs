use iced::{Point, Rectangle, Size, Vector, advanced::layout::Limits};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    #[must_use]
    pub fn perpendicular(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }

    #[must_use]
    pub fn span_of(self, size: Size) -> f32 {
        match self {
            Self::Horizontal => size.width,
            Self::Vertical => size.height,
        }
    }

    #[must_use]
    pub fn position_of(self, point: Point) -> f32 {
        match self {
            Self::Horizontal => point.x,
            Self::Vertical => point.y,
        }
    }

    #[must_use]
    pub fn along(self, distance: f32) -> Vector {
        match self {
            Self::Horizontal => Vector::new(distance, 0.0),
            Self::Vertical => Vector::new(0.0, distance),
        }
    }

    #[must_use]
    pub fn unbounded_along(self, limits: &Limits) -> Limits {
        let room = limits.max();
        let unbounded = match self {
            Self::Horizontal => Size::new(f32::INFINITY, room.height),
            Self::Vertical => Size::new(room.width, f32::INFINITY),
        };
        Limits::new(Size::ZERO, unbounded)
    }

    #[must_use]
    pub fn visible_size(self, natural: Size, room: Size) -> Size {
        match self {
            Self::Horizontal => Size::new(natural.width.min(room.width), natural.height),
            Self::Vertical => Size::new(natural.width, natural.height.min(room.height)),
        }
    }

    #[must_use]
    pub fn visible_strip(self, bounds: Rectangle, viewport: &Rectangle) -> Rectangle {
        match self {
            Self::Horizontal => Rectangle {
                x: bounds.x,
                width: bounds.width,
                ..*viewport
            },
            Self::Vertical => Rectangle {
                y: bounds.y,
                height: bounds.height,
                ..*viewport
            },
        }
    }

    #[must_use]
    pub fn start_edge(self) -> Edge {
        match self {
            Self::Horizontal => Edge::Left,
            Self::Vertical => Edge::Top,
        }
    }

    #[must_use]
    pub fn end_edge(self) -> Edge {
        match self {
            Self::Horizontal => Edge::Right,
            Self::Vertical => Edge::Bottom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    #[must_use]
    pub fn normal_axis(self) -> Axis {
        match self {
            Self::Left | Self::Right => Axis::Horizontal,
            Self::Top | Self::Bottom => Axis::Vertical,
        }
    }

    #[must_use]
    pub fn running_axis(self) -> Axis {
        self.normal_axis().perpendicular()
    }
}

#[cfg(test)]
mod tests {
    use super::{Axis, Edge};

    #[test]
    fn opposite_edges_share_an_axis() {
        [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right]
            .into_iter()
            .for_each(|edge| {
                assert_eq!(edge.opposite().opposite(), edge);
                assert_eq!(edge.opposite().normal_axis(), edge.normal_axis());
                assert_eq!(edge.running_axis().perpendicular(), edge.normal_axis());
            });
    }

    #[test]
    fn an_axis_starts_and_ends_on_its_own_edges() {
        [Axis::Horizontal, Axis::Vertical]
            .into_iter()
            .for_each(|axis| {
                assert_eq!(axis.start_edge().normal_axis(), axis);
                assert_eq!(axis.start_edge().opposite(), axis.end_edge());
            });
    }
}
