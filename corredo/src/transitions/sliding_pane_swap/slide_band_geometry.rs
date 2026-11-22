use iced::{Rectangle, Size, Vector, advanced::layout::Node};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SlideDirection {
    EnterFromBelow,
    EnterFromAbove,
}

impl SlideDirection {
    pub(super) fn between(arriving_order: u8, departing_order: u8) -> Self {
        if arriving_order >= departing_order {
            Self::EnterFromBelow
        } else {
            Self::EnterFromAbove
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlideBand {
    OwnBounds,
    AlongTopEdge {
        frame_depth_px: f32,
        float_offset_px: f32,
        window_gap_px: f32,
    },
    AlongBottomEdge {
        frame_depth_px: f32,
        float_offset_px: f32,
        window_gap_px: f32,
    },
}

pub(super) struct BandRegions {
    pub(super) whole_band: Rectangle,
    pub(super) framed_part: Rectangle,
    pub(super) floating_gap: Rectangle,
    pub(super) window_gap: Rectangle,
}

impl SlideBand {
    pub(super) fn regions(self, bounds: Rectangle, viewport: &Rectangle) -> BandRegions {
        let horizontal_strip = |top: f32, height: f32| Rectangle {
            x: bounds.x,
            width: bounds.width,
            y: top,
            height,
        };
        match self {
            Self::OwnBounds => BandRegions {
                whole_band: bounds,
                framed_part: bounds,
                floating_gap: horizontal_strip(bounds.y, 0.0),
                window_gap: horizontal_strip(bounds.y, 0.0),
            },
            Self::AlongTopEdge {
                frame_depth_px,
                float_offset_px,
                window_gap_px,
            } => {
                let band_top = viewport.y;
                let frame_top = band_top + float_offset_px;
                BandRegions {
                    whole_band: horizontal_strip(
                        band_top,
                        float_offset_px + frame_depth_px + window_gap_px,
                    ),
                    framed_part: horizontal_strip(frame_top, frame_depth_px),
                    floating_gap: horizontal_strip(band_top, float_offset_px),
                    window_gap: horizontal_strip(frame_top + frame_depth_px, window_gap_px),
                }
            }
            Self::AlongBottomEdge {
                frame_depth_px,
                float_offset_px,
                window_gap_px,
            } => {
                let frame_top = viewport.y + viewport.height - float_offset_px - frame_depth_px;
                let band_top = frame_top - window_gap_px;
                BandRegions {
                    whole_band: horizontal_strip(
                        band_top,
                        window_gap_px + frame_depth_px + float_offset_px,
                    ),
                    framed_part: horizontal_strip(frame_top, frame_depth_px),
                    floating_gap: horizontal_strip(frame_top + frame_depth_px, float_offset_px),
                    window_gap: horizontal_strip(band_top, window_gap_px),
                }
            }
        }
    }
}

struct SlideShifts {
    departing_shift: f32,
    arriving_shift: f32,
}

impl SlideDirection {
    fn shifts(
        self,
        band: Rectangle,
        departing_bounds: Rectangle,
        arriving_bounds: Rectangle,
        progress: f32,
    ) -> SlideShifts {
        let band_bottom = band.y + band.height;
        let (leaving_distance, entering_distance) = match self {
            Self::EnterFromBelow => (
                departing_bounds.y + departing_bounds.height - band.y,
                band_bottom - arriving_bounds.y,
            ),
            Self::EnterFromAbove => (
                band_bottom - departing_bounds.y,
                arriving_bounds.y + arriving_bounds.height - band.y,
            ),
        };
        let shared_travel = leaving_distance.max(entering_distance);
        let upward_travel = match self {
            Self::EnterFromBelow => -shared_travel,
            Self::EnterFromAbove => shared_travel,
        };
        SlideShifts {
            departing_shift: upward_travel * progress,
            arriving_shift: -upward_travel * (1.0 - progress),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct BandMargins {
    pub(super) above_px: f32,
    pub(super) below_px: f32,
}

pub(super) fn centred_at(node: Node, height: f32) -> Node {
    let top = (height - node.size().height) / 2.0;
    node.move_to((0.0, top))
}

pub(super) fn sliding_layout(
    arriving_node: Node,
    departing_node: Node,
    height: f32,
    band_margins: BandMargins,
    direction: SlideDirection,
    slide_progress: f32,
) -> Node {
    let arriving_width = arriving_node.size().width;
    let width = if arriving_width > 0.0 {
        arriving_width
    } else {
        departing_node.size().width
    };
    let band_in_own_coordinates = Rectangle {
        x: 0.0,
        y: -band_margins.above_px,
        width,
        height: height + band_margins.above_px + band_margins.below_px,
    };
    let SlideShifts {
        departing_shift,
        arriving_shift,
    } = direction.shifts(
        band_in_own_coordinates,
        departing_node.bounds(),
        arriving_node.bounds(),
        slide_progress,
    );
    Node::with_children(
        Size::new(width, height),
        vec![
            arriving_node.translate(Vector::new(0.0, arriving_shift)),
            departing_node.translate(Vector::new(0.0, departing_shift)),
        ],
    )
}

#[cfg(test)]
mod tests {
    use iced::Rectangle;

    use super::{SlideBand, SlideDirection};

    const BAND: Rectangle = Rectangle {
        x: 0.0,
        y: -10.0,
        width: 100.0,
        height: 50.0,
    };
    const ROW: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 20.0,
    };

    #[test]
    fn a_later_pane_enters_from_below() {
        assert_eq!(
            SlideDirection::between(2, 1),
            SlideDirection::EnterFromBelow
        );
        assert_eq!(
            SlideDirection::between(1, 2),
            SlideDirection::EnterFromAbove
        );
    }

    #[test]
    fn rows_slide_as_one_strip_and_leave_the_band() {
        [
            SlideDirection::EnterFromBelow,
            SlideDirection::EnterFromAbove,
        ]
        .into_iter()
        .for_each(|direction| {
            let at_start = direction.shifts(BAND, ROW, ROW, 0.0);
            let halfway = direction.shifts(BAND, ROW, ROW, 0.5);
            let at_end = direction.shifts(BAND, ROW, ROW, 1.0);
            assert!(at_start.departing_shift.abs() < f32::EPSILON);
            assert!(at_end.arriving_shift.abs() < f32::EPSILON);
            let strip_offset = at_start.arriving_shift - at_start.departing_shift;
            assert!((halfway.arriving_shift - halfway.departing_shift - strip_offset).abs() < 0.01);
            let arriving_at_start_top = ROW.y + at_start.arriving_shift;
            let departing_at_end_top = ROW.y + at_end.departing_shift;
            let band_bottom = BAND.y + BAND.height;
            match direction {
                SlideDirection::EnterFromBelow => {
                    assert!(arriving_at_start_top >= band_bottom - 0.01);
                    assert!(departing_at_end_top + ROW.height <= BAND.y + 0.01);
                }
                SlideDirection::EnterFromAbove => {
                    assert!(arriving_at_start_top + ROW.height <= BAND.y + 0.01);
                    assert!(departing_at_end_top >= band_bottom - 0.01);
                }
            }
        });
    }

    const SCREEN: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 600.0,
    };

    #[test]
    fn the_band_reaches_through_the_window_gap() {
        let bottom_regions = SlideBand::AlongBottomEdge {
            frame_depth_px: 30.0,
            float_offset_px: 4.0,
            window_gap_px: 16.0,
        }
        .regions(ROW, &SCREEN);
        assert!((bottom_regions.framed_part.y - 566.0).abs() < 0.01);
        assert!((bottom_regions.whole_band.y - 550.0).abs() < 0.01);
        assert!((bottom_regions.whole_band.height - 50.0).abs() < 0.01);
        assert!((bottom_regions.window_gap.y - 550.0).abs() < 0.01);
        let top_regions = SlideBand::AlongTopEdge {
            frame_depth_px: 30.0,
            float_offset_px: 4.0,
            window_gap_px: 16.0,
        }
        .regions(ROW, &SCREEN);
        assert!(top_regions.whole_band.y.abs() < 0.01);
        assert!((top_regions.whole_band.height - 50.0).abs() < 0.01);
        assert!((top_regions.window_gap.y - 34.0).abs() < 0.01);
    }
}
