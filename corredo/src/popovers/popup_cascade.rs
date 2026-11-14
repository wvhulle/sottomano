use iced::Rectangle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupGrowth {
    Downward,
    Upward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeDirection {
    Rightward,
    Leftward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CascadeOrientation {
    pub direction: CascadeDirection,
    pub growth: PopupGrowth,
}

impl CascadeOrientation {
    #[must_use]
    pub fn toward_room(anchor: Rectangle, growth: PopupGrowth, surface_width: f32) -> Self {
        let direction = if anchor.center_x() > surface_width / 2.0 {
            CascadeDirection::Leftward
        } else {
            CascadeDirection::Rightward
        };
        CascadeOrientation { direction, growth }
    }
}
