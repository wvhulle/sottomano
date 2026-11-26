use std::{borrow::Cow, time::Duration};

use chrono::{Locale, TimeDelta};
use corredo::{
    gauges::{
        ClockScale, ClockSpan, DurationTimeline, EventGlyph, EventMeasure, RailPresence,
        SpanCaption, SpanEdges, SpanInk, SpanWeight, TimePrecision, TimelineEvent, TimelineSpan,
        WallClockTimeline,
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
    widgets::{localized_durations::duration_text, localized_timeline_text::LocalizedTimelineText},
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

fn logarithmic_knee(stages: &[TimedStage]) -> Duration {
    stages
        .iter()
        .map(|timed| timed.reached_after)
        .filter(|reached_after| !reached_after.is_zero())
        .min()
        .unwrap_or(EMPTY_CHAIN_SPAN)
}

fn stage_event(timed: TimedStage, clock: Clock) -> TimelineEvent {
    TimelineEvent {
        at: clock.advanced(timed.reached_after).utc(),
        precision: match timed.timing {
            StageTiming::Awake => TimePrecision::Exact,
            StageTiming::AsleepUntilHibernate => TimePrecision::Approximate,
        },
        glyph: EventGlyph::Svg(pause_glyph(timed.stage).svg_data()),
        title: Cow::Borrowed(pause_title(timed.stage)),
        measure: EventMeasure::OffsetFromAnchor(chrono_time_delta(timed.offset)),
        ink: SpanInk::Strong,
        rail_presence: RailPresence::OnCompactRail,
    }
}

fn last_input_event(clock: Clock) -> TimelineEvent {
    TimelineEvent {
        at: clock.utc(),
        precision: TimePrecision::Exact,
        glyph: EventGlyph::Plain,
        title: Cow::Borrowed("Last input"),
        measure: EventMeasure::Unmeasured,
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

fn chrono_time_delta(duration: Duration) -> TimeDelta {
    TimeDelta::from_std(duration).expect("an idle chain lasts far less than TimeDelta's range")
}

pub(super) fn first_stage_text(chain: &ResolvedIdleChain, locale: Locale) -> String {
    timed_stages(chain).first().map_or_else(
        || "No idle actions".to_owned(),
        |first_stage| {
            format!(
                "{} after {} idle",
                pause_title(first_stage.stage),
                duration_text(first_stage.reached_after, locale)
            )
        },
    )
}

pub(super) fn idle_timeline(
    chain: &ResolvedIdleChain,
    clock: Clock,
    locale: Locale,
    label_face: TextFace,
) -> DurationTimeline {
    let stages = timed_stages(chain);
    let clock_spans = stage_spans(&stages).into_iter().map(|span| ClockSpan {
        from: clock.advanced(span.from).utc(),
        until: clock.advanced(span.until).utc(),
        ink: span.ink,
        weight: span.weight,
        edges: SpanEdges::Unmarked,
        caption: SpanCaption::Hidden,
    });
    WallClockTimeline::new(clock.utc())
        .minimum_span(chrono_time_delta(EMPTY_CHAIN_SPAN))
        .end_margin_fraction(SPAN_END_MARGIN_FRACTION)
        .scale(ClockScale::Logarithmic {
            knee: chrono_time_delta(logarithmic_knee(&stages)),
        })
        .clock_spans(clock_spans)
        .events(
            [last_input_event(clock)]
                .into_iter()
                .chain(stages.iter().map(|timed| stage_event(*timed, clock))),
        )
        .duration_timeline(label_face, &LocalizedTimelineText { locale })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Locale;
    use corredo::gauges::{SpanInk, SpanWeight, TimelineSpan};
    use sottomano_desktop_sources::login_session_presence::idle_session_actions::IdleAction;

    use super::{StageTiming, TimedStage, first_stage_text, stage_spans, timed_stages};
    use crate::panel_menus::session_menu::idle_pause_groups::{
        ResolvedIdleChain, StageSetting, SuspendThenHibernateSupport,
    };

    fn minutes(count: u64) -> StageSetting {
        StageSetting::After(Duration::from_mins(count))
    }

    fn resolved(settings: [StageSetting; 5]) -> ResolvedIdleChain {
        ResolvedIdleChain::resolve(settings, SuspendThenHibernateSupport::Permitted)
    }

    #[test]
    fn the_row_names_the_first_stage_and_its_idle_time() {
        let chain = resolved([
            minutes(5),
            minutes(0),
            minutes(15),
            minutes(180),
            minutes(60),
        ]);
        assert_eq!(
            first_stage_text(&chain, Locale::en_US),
            "Lock after 5m idle"
        );
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
