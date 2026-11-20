use std::time::Duration;

use chrono::Locale;
use corredo::{
    gauges::{
        DurationTimeline, NowCursor, RailPresence, SpanInk, SpanWeight, TimelineAxis, TimelineMark,
        TimelineScale, TimelineSpan,
    },
    theming::TextFace,
};
use sottomano_desktop_sources::login_session_presence::idle_session_actions::IdleAction;

use super::{
    ResolvedIdleChain, StageSchedule, pause_blocker_indicators::pause_title,
    pause_group_elements::pause_glyph,
};
use crate::{
    clock::Clock,
    widgets::{localized_durations::duration_text, localized_time::LocalizedTimeFormat},
};

const EMPTY_CHAIN_SPAN: Duration = Duration::from_hours(1);
const SPAN_END_MARGIN_FRACTION: f64 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageTiming {
    Awake,
    AsleepUntilHibernate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TimedStage {
    stage: IdleAction,
    counted_from: Duration,
    reached_after: Duration,
    offset: Duration,
    timing: StageTiming,
}

fn timed_stages(chain: &ResolvedIdleChain) -> Vec<TimedStage> {
    let mut previous_idle_for = Duration::ZERO;
    IdleAction::ALL
        .into_iter()
        .filter_map(|stage| match chain.stage(stage).schedule {
            StageSchedule::AfterIdle { idle_for, .. } => {
                let counted_from = previous_idle_for;
                let offset = idle_for
                    .checked_sub(counted_from)
                    .expect("idle times along the chain only grow");
                previous_idle_for = idle_for;
                Some(TimedStage {
                    stage,
                    counted_from,
                    reached_after: idle_for,
                    offset,
                    timing: StageTiming::Awake,
                })
            }
            StageSchedule::AsleepAfterStandby { asleep_for } => Some(TimedStage {
                stage,
                counted_from: previous_idle_for,
                reached_after: previous_idle_for + asleep_for,
                offset: asleep_for,
                timing: StageTiming::AsleepUntilHibernate,
            }),
            StageSchedule::Off
            | StageSchedule::Unavailable
            | StageSchedule::UnreachableWhileAsleep
            | StageSchedule::ZeroWithoutEarlierStage => None,
        })
        .collect()
}

fn timeline_span_end(stages: &[TimedStage]) -> Duration {
    stages
        .iter()
        .map(|timed| timed.reached_after)
        .max()
        .filter(|latest| !latest.is_zero())
        .map_or(EMPTY_CHAIN_SPAN, |latest| {
            latest.mul_f64(1.0 + SPAN_END_MARGIN_FRACTION)
        })
}

fn logarithmic_knee(stages: &[TimedStage]) -> Duration {
    stages
        .iter()
        .map(|timed| timed.reached_after)
        .filter(|reached_after| !reached_after.is_zero())
        .min()
        .unwrap_or(EMPTY_CHAIN_SPAN)
}

fn stage_mark(timed: TimedStage, clock: Clock, locale: Locale) -> TimelineMark {
    let reached_at = LocalizedTimeFormat::HourAndMinute
        .format(&clock.advanced(timed.reached_after).local(), locale);
    let offset_text = duration_text(timed.offset, locale);
    let stage_title = pause_title(timed.stage);
    let (time_text, detail_text) = match timed.timing {
        StageTiming::Awake => (reached_at, format!("{stage_title} · +{offset_text}")),
        StageTiming::AsleepUntilHibernate => (
            format!("~{reached_at}"),
            format!("{stage_title} · after {offset_text} asleep"),
        ),
    };
    TimelineMark {
        at: timed.reached_after,
        glyph_svgs: vec![pause_glyph(timed.stage).svg_data()],
        time_text,
        detail_text,
        ink: SpanInk::Strong,
        rail_presence: RailPresence::OnCompactRail,
    }
}

fn last_input_mark(clock: Clock, locale: Locale) -> TimelineMark {
    TimelineMark {
        at: Duration::ZERO,
        glyph_svgs: Vec::new(),
        time_text: LocalizedTimeFormat::HourAndMinute.format(&clock.local(), locale),
        detail_text: "Now · last input".to_owned(),
        ink: SpanInk::Strong,
        rail_presence: RailPresence::LabeledColumnOnly,
    }
}

fn stage_spans(stages: &[TimedStage]) -> Vec<TimelineSpan> {
    let awake_until = stages
        .iter()
        .filter(|timed| timed.timing == StageTiming::Awake)
        .map(|timed| timed.reached_after)
        .max()
        .unwrap_or(Duration::ZERO);
    let awake_span = TimelineSpan {
        from: Duration::ZERO,
        until: awake_until,
        ink: SpanInk::Strong,
        weight: SpanWeight::Thin,
    };
    let asleep_spans = stages
        .iter()
        .filter(|timed| timed.timing == StageTiming::AsleepUntilHibernate)
        .map(|timed| TimelineSpan {
            from: timed.counted_from,
            until: timed.reached_after,
            ink: SpanInk::Strong,
            weight: SpanWeight::Thick,
        });
    [awake_span].into_iter().chain(asleep_spans).collect()
}

pub(super) fn idle_timeline(
    chain: &ResolvedIdleChain,
    clock: Clock,
    locale: Locale,
    label_face: TextFace,
) -> DurationTimeline {
    let stages = timed_stages(chain);
    DurationTimeline::new(
        TimelineAxis {
            span_end: timeline_span_end(&stages),
            scale: TimelineScale::Logarithmic {
                knee: logarithmic_knee(&stages),
            },
        },
        label_face,
    )
    .spans(stage_spans(&stages))
    .marks(
        [last_input_mark(clock, locale)]
            .into_iter()
            .chain(stages.iter().map(|timed| stage_mark(*timed, clock, locale)))
            .collect(),
    )
    .now_cursor(NowCursor::Shown(Duration::ZERO))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use corredo::gauges::{SpanInk, SpanWeight, TimelineSpan};
    use sottomano_desktop_sources::login_session_presence::idle_session_actions::IdleAction;

    use super::{StageTiming, TimedStage, stage_spans, timed_stages};
    use crate::indicator_groups::idle_pause_groups::{
        ResolvedIdleChain, StageSetting, SuspendThenHibernateSupport,
    };

    fn minutes(count: u64) -> StageSetting {
        StageSetting::After(Duration::from_mins(count))
    }

    fn resolved(settings: [StageSetting; 5]) -> ResolvedIdleChain {
        ResolvedIdleChain::resolve(settings, SuspendThenHibernateSupport::Permitted)
    }

    #[test]
    fn hibernate_after_standby_is_timed_by_the_time_asleep() {
        let chain = resolved([
            minutes(5),
            minutes(0),
            minutes(15),
            minutes(180),
            minutes(60),
        ]);
        let stages = timed_stages(&chain);
        assert_eq!(
            stages,
            vec![
                TimedStage {
                    stage: IdleAction::Lock,
                    counted_from: Duration::ZERO,
                    reached_after: Duration::from_mins(5),
                    offset: Duration::from_mins(5),
                    timing: StageTiming::Awake,
                },
                TimedStage {
                    stage: IdleAction::ScreenOff,
                    counted_from: Duration::from_mins(5),
                    reached_after: Duration::from_mins(5),
                    offset: Duration::ZERO,
                    timing: StageTiming::Awake,
                },
                TimedStage {
                    stage: IdleAction::Sleep,
                    counted_from: Duration::from_mins(5),
                    reached_after: Duration::from_mins(20),
                    offset: Duration::from_mins(15),
                    timing: StageTiming::Awake,
                },
                TimedStage {
                    stage: IdleAction::Hibernate,
                    counted_from: Duration::from_mins(20),
                    reached_after: Duration::from_mins(200),
                    offset: Duration::from_mins(180),
                    timing: StageTiming::AsleepUntilHibernate,
                },
            ]
        );
        assert_eq!(
            stage_spans(&stages)[1],
            TimelineSpan {
                from: Duration::from_mins(20),
                until: Duration::from_mins(200),
                ink: SpanInk::Strong,
                weight: SpanWeight::Thick,
            }
        );
    }

    #[test]
    fn without_standby_hibernate_is_an_awake_stage_and_nothing_sleeps() {
        let chain = resolved([
            minutes(5),
            StageSetting::Off,
            StageSetting::Off,
            minutes(30),
            StageSetting::Off,
        ]);
        let stages = timed_stages(&chain);
        assert!(
            stages
                .iter()
                .all(|timed| timed.timing == StageTiming::Awake)
        );
        assert_eq!(stage_spans(&stages).len(), 1);
    }
}
