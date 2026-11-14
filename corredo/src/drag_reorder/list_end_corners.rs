use iced::border::Radius;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListEndCorners {
    frame_corners: Radius,
    last_position: Option<usize>,
}

impl ListEndCorners {
    #[must_use]
    pub fn new(item_count: usize, frame_corners: Radius) -> Self {
        Self {
            frame_corners,
            last_position: item_count.checked_sub(1),
        }
    }

    #[must_use]
    pub fn radius_at(self, position: usize) -> Radius {
        let frame = self.frame_corners;
        let top = if position == 0 {
            Radius {
                top_left: frame.top_left,
                top_right: frame.top_right,
                ..Radius::default()
            }
        } else {
            Radius::default()
        };
        if Some(position) == self.last_position {
            Radius {
                bottom_left: frame.bottom_left,
                bottom_right: frame.bottom_right,
                ..top
            }
        } else {
            top
        }
    }
}

#[cfg(test)]
mod tests {
    use iced::border::Radius;

    use super::ListEndCorners;

    #[test]
    fn only_the_frame_corners_next_to_an_end_item_are_kept() {
        let corners = ListEndCorners::new(3, Radius::from(8.0));
        assert_eq!(corners.radius_at(0), Radius::default().top(8.0));
        assert_eq!(corners.radius_at(1), Radius::default());
        assert_eq!(corners.radius_at(2), Radius::default().bottom(8.0));
    }

    #[test]
    fn square_frame_corners_at_the_screen_edge_stay_square() {
        let docked_left = Radius::default().right(8.0);
        let corners = ListEndCorners::new(2, docked_left);
        assert_eq!(corners.radius_at(0), Radius::default().top_right(8.0));
        assert_eq!(corners.radius_at(1), Radius::default().bottom_right(8.0));
    }
}
