use iced_widget::core::{Point, Rectangle, Size};

use crate::{
    diagram_anchor::DiagramAnchor,
    sdl_gamepad_artwork::{BODY_SIZE, anchor_point},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CalloutSide {
    Left,
    Right,
}

impl CalloutSide {
    pub(crate) fn of(anchor: DiagramAnchor) -> Self {
        if anchor_point(anchor).x < BODY_SIZE.width / 2.0 {
            CalloutSide::Left
        } else {
            CalloutSide::Right
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DiagramMetrics {
    pub scale: f32,
    pub row_height: f32,
    pub row_gap: f32,
    pub gutter: f32,
    pub stub: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LeaderLine {
    pub label_end: Point,
    pub bend: Point,
    pub anchor: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CalloutLayout {
    pub tops: Vec<f32>,
    pub body: Rectangle,
    pub art_size: Size,
    pub leaders: Vec<LeaderLine>,
}

fn desired_top(anchor: DiagramAnchor, metrics: DiagramMetrics) -> f32 {
    anchor_point(anchor).y * metrics.scale - metrics.row_height / 2.0
}

fn stacked_tops(
    anchors: &[DiagramAnchor],
    side: CalloutSide,
    metrics: DiagramMetrics,
) -> Vec<(usize, f32)> {
    let mut members: Vec<usize> = (0..anchors.len())
        .filter(|&index| CalloutSide::of(anchors[index]) == side)
        .collect();
    members.sort_by(|&a, &b| {
        desired_top(anchors[a], metrics).total_cmp(&desired_top(anchors[b], metrics))
    });
    let pitch = metrics.row_height + metrics.row_gap;
    let block_top = |block: &[usize]| {
        let mean = block
            .iter()
            .enumerate()
            .map(|(offset, &index)| desired_top(anchors[index], metrics) - offset as f32 * pitch)
            .sum::<f32>()
            / block.len() as f32;
        mean.max(0.0)
    };
    let mut blocks: Vec<(Vec<usize>, f32)> = Vec::new();
    for index in members {
        blocks.push((vec![index], desired_top(anchors[index], metrics).max(0.0)));
        while let [.., (previous, previous_top), (_, last_top)] = blocks.as_slice()
            && previous_top + previous.len() as f32 * pitch > *last_top
        {
            let (last, _) = blocks.pop().unwrap_or_else(|| unreachable!());
            let (previous, previous_top) = blocks.last_mut().unwrap_or_else(|| unreachable!());
            previous.extend(last);
            *previous_top = block_top(previous);
        }
    }
    blocks
        .into_iter()
        .flat_map(|(block, top)| {
            block
                .into_iter()
                .enumerate()
                .map(move |(offset, index)| (index, top + offset as f32 * pitch))
        })
        .collect()
}

pub(crate) fn callout_layout(anchors: &[DiagramAnchor], metrics: DiagramMetrics) -> CalloutLayout {
    let body = Rectangle::new(Point::new(metrics.gutter, 0.0), BODY_SIZE * metrics.scale);
    let mut tops = vec![0.0; anchors.len()];
    for side in [CalloutSide::Left, CalloutSide::Right] {
        for (index, top) in stacked_tops(anchors, side, metrics) {
            tops[index] = top;
        }
    }
    let art_width = body.width + 2.0 * metrics.gutter;
    let leaders = anchors
        .iter()
        .zip(&tops)
        .map(|(&anchor, &top)| {
            let row = top + metrics.row_height / 2.0;
            let point = anchor_point(anchor);
            let (edge, stub) = match CalloutSide::of(anchor) {
                CalloutSide::Left => (0.0, metrics.stub),
                CalloutSide::Right => (art_width, -metrics.stub),
            };
            LeaderLine {
                label_end: Point::new(edge, row),
                bend: Point::new(edge + stub, row),
                anchor: Point::new(
                    body.x + point.x * metrics.scale,
                    body.y + point.y * metrics.scale,
                ),
            }
        })
        .collect();
    let height = tops
        .iter()
        .map(|top| top + metrics.row_height)
        .fold(body.height, f32::max);
    CalloutLayout {
        tops,
        body,
        art_size: Size::new(art_width, height),
        leaders,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_anchor::{DiagramButton, DiagramStick};

    const METRICS: DiagramMetrics = DiagramMetrics {
        scale: 0.6,
        row_height: 18.0,
        row_gap: 2.0,
        gutter: 24.0,
        stub: 6.0,
    };

    fn every_anchor() -> Vec<DiagramAnchor> {
        DiagramButton::ALL
            .into_iter()
            .map(DiagramAnchor::from)
            .chain([DiagramStick::Left, DiagramStick::Right].map(DiagramAnchor::from))
            .collect()
    }

    #[test]
    fn rows_on_one_side_never_overlap() {
        let anchors = every_anchor();
        let layout = callout_layout(&anchors, METRICS);
        for side in [CalloutSide::Left, CalloutSide::Right] {
            let mut tops: Vec<f32> = anchors
                .iter()
                .zip(&layout.tops)
                .filter(|(anchor, _)| CalloutSide::of(**anchor) == side)
                .map(|(_, &top)| top)
                .collect();
            tops.sort_by(f32::total_cmp);
            for pair in tops.windows(2) {
                assert!(pair[1] - pair[0] >= METRICS.row_height + METRICS.row_gap - 0.01);
            }
        }
    }

    #[test]
    fn rows_keep_the_vertical_order_of_their_buttons() {
        let anchors = [
            DiagramAnchor::from(DiagramButton::DpadDown),
            DiagramAnchor::from(DiagramButton::DpadUp),
        ];
        let layout = callout_layout(&anchors, METRICS);
        assert!(layout.tops[1] < layout.tops[0]);
    }

    #[test]
    fn a_lone_label_sits_level_with_its_button() {
        let anchors = [DiagramAnchor::from(DiagramButton::North)];
        let layout = callout_layout(&anchors, METRICS);
        let leader = layout.leaders[0];
        assert!((leader.label_end.y - leader.anchor.y).abs() < 0.01);
    }

    #[test]
    fn left_leaders_start_at_the_left_edge_and_right_leaders_at_the_right() {
        let anchors = [
            DiagramAnchor::from(DiagramButton::DpadLeft),
            DiagramAnchor::from(DiagramButton::South),
        ];
        let layout = callout_layout(&anchors, METRICS);
        assert!(layout.leaders[0].label_end.x.abs() < f32::EPSILON);
        assert!((layout.leaders[1].label_end.x - layout.art_size.width).abs() < f32::EPSILON);
    }

    #[test]
    fn crowded_rows_grow_the_diagram_below_the_image() {
        let anchors = [DiagramAnchor::from(DiagramButton::DpadDown); 12];
        let layout = callout_layout(&anchors, METRICS);
        assert!(layout.art_size.height > layout.body.height);
    }
}
