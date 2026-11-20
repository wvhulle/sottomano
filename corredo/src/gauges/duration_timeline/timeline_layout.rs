use std::time::Duration;

const GLYPH_WIDTH_PER_TEXT_SIZE: f32 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineScale {
    Linear,
    Logarithmic { knee: Duration },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineAxis {
    pub span_end: Duration,
    pub scale: TimelineScale,
}

impl TimelineAxis {
    #[must_use]
    pub fn axis_fraction(self, offset: Duration) -> f32 {
        let clamped_offset = offset.min(self.span_end).as_secs_f64();
        let span_seconds = self.span_end.as_secs_f64();
        if span_seconds <= 0.0 {
            return 0.0;
        }
        let fraction = match self.scale {
            TimelineScale::Linear => clamped_offset / span_seconds,
            TimelineScale::Logarithmic { knee } => {
                let knee_seconds = knee.as_secs_f64().max(f64::EPSILON);
                (clamped_offset / knee_seconds).ln_1p() / (span_seconds / knee_seconds).ln_1p()
            }
        };
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a fraction in 0..=1 fits an f32 for pixel placement"
        )]
        let pixel_fraction = fraction.clamp(0.0, 1.0) as f32;
        pixel_fraction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanInk {
    Faint,
    Strong,
    Accent,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanWeight {
    Thin,
    Thick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineSpan {
    pub from: Duration,
    pub until: Duration,
    pub ink: SpanInk,
    pub weight: SpanWeight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineMark {
    pub at: Duration,
    pub glyph_svgs: Vec<&'static [u8]>,
    pub time_text: String,
    pub detail_text: String,
    pub ink: SpanInk,
    pub rail_presence: RailPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailPresence {
    OnCompactRail,
    LabeledColumnOnly,
}

pub(super) fn marks_in_time_order(marks: Vec<TimelineMark>) -> Vec<TimelineMark> {
    let mut ordered_marks = marks;
    ordered_marks.sort_by_key(|mark| mark.at);
    ordered_marks
}

pub(super) fn glyphs_merged_by_time(marks: &[TimelineMark]) -> Vec<TimelineMark> {
    marks
        .iter()
        .filter(|mark| mark.rail_presence == RailPresence::OnCompactRail)
        .cloned()
        .fold(Vec::new(), |mut merged: Vec<TimelineMark>, mark| {
            match merged.last_mut() {
                Some(previous) if previous.at == mark.at => {
                    previous.glyph_svgs.extend(mark.glyph_svgs);
                }
                Some(_) | None => merged.push(mark),
            }
            merged
        })
}

pub(super) fn estimated_text_width(label_text: &str, text_size_px: f32) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "label lengths are a few dozen characters"
    )]
    let character_count = label_text.chars().count() as f32;
    character_count * text_size_px * GLYPH_WIDTH_PER_TEXT_SIZE
}

pub(super) fn center_inside(center: f32, extent: f32, available_extent: f32) -> f32 {
    let half_extent = extent / 2.0;
    if available_extent <= extent {
        return available_extent / 2.0;
    }
    center.clamp(half_extent, available_extent - half_extent)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PixelRun {
    pub start: f32,
    pub end: f32,
}

pub(super) fn runs_around_gaps(whole_run: PixelRun, gaps: &[PixelRun]) -> Vec<PixelRun> {
    let mut ordered_gaps = gaps.to_vec();
    ordered_gaps.sort_by(|earlier, later| earlier.start.total_cmp(&later.start));
    let mut visible_runs = Vec::new();
    let mut run_start = whole_run.start;
    for gap in ordered_gaps {
        if gap.end <= run_start || gap.start >= whole_run.end {
            continue;
        }
        if gap.start > run_start {
            visible_runs.push(PixelRun {
                start: run_start,
                end: gap.start,
            });
        }
        run_start = run_start.max(gap.end);
    }
    if run_start < whole_run.end {
        visible_runs.push(PixelRun {
            start: run_start,
            end: whole_run.end,
        });
    }
    visible_runs
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        PixelRun, RailPresence, SpanInk, TimelineAxis, TimelineMark, TimelineScale,
        glyphs_merged_by_time, marks_in_time_order, runs_around_gaps,
    };

    const ONE_MINUTE_KNEE: TimelineScale = TimelineScale::Logarithmic {
        knee: Duration::from_mins(1),
    };

    fn axis_with(scale: TimelineScale) -> TimelineAxis {
        TimelineAxis {
            span_end: Duration::from_hours(3),
            scale,
        }
    }

    #[test]
    fn both_scales_run_from_zero_to_one_and_keep_order() {
        for scale in [TimelineScale::Linear, ONE_MINUTE_KNEE] {
            let axis = axis_with(scale);
            assert!(axis.axis_fraction(Duration::ZERO).abs() < f32::EPSILON);
            assert!((axis.axis_fraction(Duration::from_hours(3)) - 1.0).abs() < 1e-6);
            assert!((axis.axis_fraction(Duration::from_hours(9)) - 1.0).abs() < 1e-6);
            let fractions: Vec<f32> = [1, 5, 20, 60, 120]
                .map(|minutes| axis.axis_fraction(Duration::from_mins(minutes)))
                .to_vec();
            assert!(fractions.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

    #[test]
    fn the_logarithmic_scale_gives_minutes_more_room_than_hours() {
        let five_minutes = Duration::from_mins(5);
        assert!(
            axis_with(ONE_MINUTE_KNEE).axis_fraction(five_minutes)
                > 3.0 * axis_with(TimelineScale::Linear).axis_fraction(five_minutes)
        );
    }

    const LOCK_SVG: &[u8] = b"<svg id='lock'/>";
    const SCREEN_SVG: &[u8] = b"<svg id='screen'/>";
    const STANDBY_SVG: &[u8] = b"<svg id='standby'/>";

    fn mark_at(minutes: u64, glyph_svgs: &[&'static [u8]], detail_text: &str) -> TimelineMark {
        TimelineMark {
            at: Duration::from_mins(minutes),
            glyph_svgs: glyph_svgs.to_vec(),
            time_text: String::new(),
            detail_text: detail_text.to_owned(),
            ink: SpanInk::Strong,
            rail_presence: RailPresence::OnCompactRail,
        }
    }

    #[test]
    fn the_rail_joins_the_glyphs_of_marks_at_the_same_time() {
        let ordered = marks_in_time_order(vec![
            mark_at(20, &[STANDBY_SVG], "Standby"),
            mark_at(5, &[LOCK_SVG], "Lock"),
            mark_at(5, &[SCREEN_SVG], "Screen off"),
        ]);
        assert_eq!(
            glyphs_merged_by_time(&ordered),
            vec![
                mark_at(5, &[LOCK_SVG, SCREEN_SVG], "Lock"),
                mark_at(20, &[STANDBY_SVG], "Standby")
            ]
        );
    }

    #[test]
    fn the_rail_skips_the_glyphs_on_it() {
        let runs = runs_around_gaps(
            PixelRun {
                start: 0.0,
                end: 100.0,
            },
            &[
                PixelRun {
                    start: 60.0,
                    end: 70.0,
                },
                PixelRun {
                    start: 20.0,
                    end: 30.0,
                },
                PixelRun {
                    start: 95.0,
                    end: 120.0,
                },
            ],
        );
        assert_eq!(
            runs,
            vec![
                PixelRun {
                    start: 0.0,
                    end: 20.0
                },
                PixelRun {
                    start: 30.0,
                    end: 60.0
                },
                PixelRun {
                    start: 70.0,
                    end: 95.0
                },
            ]
        );
    }
}
