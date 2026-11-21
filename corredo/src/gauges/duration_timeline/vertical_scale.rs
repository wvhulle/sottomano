use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct VerticalMetrics {
    pub row_height_px: f32,
    pub mark_spacing_px: f32,
    pub pixels_per_minute: f32,
    pub longest_gap_px: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ScaleAnchor {
    at: Duration,
    y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct VerticalScale {
    anchors: Vec<ScaleAnchor>,
    pub mark_ys: Vec<f32>,
    pub compressed_gap_ys: Vec<f32>,
    pub total_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorKind {
    Edge,
    Mark,
}

fn minutes_between(earlier: Duration, later: Duration) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a timeline spans hours, far inside f32 precision for pixels"
    )]
    let minutes = (later
        .checked_sub(earlier)
        .expect("anchors are laid out in time order")
        .as_secs_f64()
        / 60.0) as f32;
    minutes
}

impl VerticalScale {
    pub(super) fn laid_out(
        ordered_mark_times: &[Duration],
        span_end: Duration,
        metrics: VerticalMetrics,
    ) -> Self {
        let span_end = ordered_mark_times
            .iter()
            .copied()
            .fold(span_end, Duration::max);
        let timed_anchors = [(Duration::ZERO, AnchorKind::Edge)]
            .into_iter()
            .chain(
                ordered_mark_times
                    .iter()
                    .map(|mark_time| (*mark_time, AnchorKind::Mark)),
            )
            .chain([(span_end, AnchorKind::Edge)]);
        let mut anchors: Vec<ScaleAnchor> = Vec::new();
        let mut mark_ys = Vec::new();
        let mut compressed_gap_ys = Vec::new();
        let mut previous_kind = AnchorKind::Edge;
        for (at, kind) in timed_anchors {
            let anchor_height = match anchors.last() {
                None => metrics.row_height_px / 2.0,
                Some(previous) => {
                    let natural_gap = minutes_between(previous.at, at) * metrics.pixels_per_minute;
                    let capped_gap = natural_gap.min(metrics.longest_gap_px);
                    if natural_gap > metrics.longest_gap_px {
                        compressed_gap_ys.push(previous.y + capped_gap / 2.0);
                    }
                    let readable_gap = match (previous_kind, kind) {
                        (AnchorKind::Mark, AnchorKind::Mark) => {
                            capped_gap.max(metrics.mark_spacing_px)
                        }
                        (AnchorKind::Edge | AnchorKind::Mark, _) => capped_gap,
                    };
                    previous.y + readable_gap
                }
            };
            if kind == AnchorKind::Mark {
                mark_ys.push(anchor_height);
            }
            anchors.push(ScaleAnchor {
                at,
                y: anchor_height,
            });
            previous_kind = kind;
        }
        let total_height = anchors.last().map_or(metrics.row_height_px, |last| {
            last.y + metrics.row_height_px / 2.0
        });
        Self {
            anchors,
            mark_ys,
            compressed_gap_ys,
            total_height,
        }
    }

    pub(super) fn vertical_position_at(&self, at: Duration) -> f32 {
        let following_index = self
            .anchors
            .iter()
            .position(|anchor| anchor.at >= at)
            .unwrap_or(self.anchors.len() - 1);
        let following = self.anchors[following_index];
        let Some(preceding) = following_index
            .checked_sub(1)
            .map(|preceding_index| self.anchors[preceding_index])
        else {
            return following.y;
        };
        if following.at <= preceding.at || at >= following.at {
            return following.y;
        }
        let progress =
            minutes_between(preceding.at, at) / minutes_between(preceding.at, following.at);
        preceding.y + progress * (following.y - preceding.y)
    }

    #[must_use]
    pub(super) fn newest_first(self) -> Self {
        let flip = |height_from_top: f32| self.total_height - height_from_top;
        Self {
            anchors: self
                .anchors
                .iter()
                .map(|anchor| ScaleAnchor {
                    at: anchor.at,
                    y: flip(anchor.y),
                })
                .collect(),
            mark_ys: self.mark_ys.iter().copied().map(flip).collect(),
            compressed_gap_ys: self.compressed_gap_ys.iter().copied().map(flip).collect(),
            total_height: self.total_height,
        }
    }

    pub(super) fn top_y(&self) -> f32 {
        self.anchors
            .iter()
            .map(|anchor| anchor.y)
            .fold(f32::INFINITY, f32::min)
            .min(self.total_height)
    }

    pub(super) fn bottom_y(&self) -> f32 {
        self.anchors
            .iter()
            .map(|anchor| anchor.y)
            .fold(0.0, f32::max)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{VerticalMetrics, VerticalScale};

    const METRICS: VerticalMetrics = VerticalMetrics {
        row_height_px: 20.0,
        mark_spacing_px: 30.0,
        pixels_per_minute: 2.0,
        longest_gap_px: 80.0,
    };

    fn minutes(count: u64) -> Duration {
        Duration::from_mins(count)
    }

    #[test]
    fn short_gaps_keep_their_clock_proportions() {
        let scale = VerticalScale::laid_out(&[minutes(15), minutes(35)], minutes(40), METRICS);
        assert_eq!(scale.mark_ys, vec![10.0 + 30.0, 10.0 + 30.0 + 40.0]);
        assert!(scale.compressed_gap_ys.is_empty());
    }

    #[test]
    fn marks_at_the_same_time_keep_a_spaced_line_each() {
        let scale = VerticalScale::laid_out(&[minutes(15), minutes(15)], minutes(20), METRICS);
        assert_eq!(scale.mark_ys, vec![40.0, 70.0]);
    }

    #[test]
    fn long_gaps_are_capped_and_marked_as_compressed() {
        let scale = VerticalScale::laid_out(&[minutes(10), minutes(200)], minutes(210), METRICS);
        assert_eq!(scale.mark_ys, vec![30.0, 110.0]);
        assert_eq!(scale.compressed_gap_ys, vec![70.0]);
        assert!((scale.total_height - (110.0 + 20.0 + 10.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn newest_first_mirrors_every_height_and_keeps_interpolation() {
        let oldest_first =
            VerticalScale::laid_out(&[minutes(10), minutes(200)], minutes(210), METRICS);
        let total_height = oldest_first.total_height;
        let newest_first = oldest_first.clone().newest_first();
        assert_eq!(
            newest_first.mark_ys,
            vec![total_height - 30.0, total_height - 110.0]
        );
        assert!(
            (newest_first.vertical_position_at(minutes(105)) - (total_height - 70.0)).abs() < 1e-3
        );
        assert!(newest_first.top_y() < newest_first.bottom_y());
    }

    #[test]
    fn times_between_marks_interpolate_along_the_drawn_gap() {
        let scale = VerticalScale::laid_out(&[minutes(10), minutes(200)], minutes(210), METRICS);
        assert!((scale.vertical_position_at(minutes(105)) - 70.0).abs() < 1e-3);
        assert!((scale.vertical_position_at(minutes(5)) - 20.0).abs() < 1e-3);
    }
}
