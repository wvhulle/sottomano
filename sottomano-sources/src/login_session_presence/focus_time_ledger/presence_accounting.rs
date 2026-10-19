use chrono::{DateTime, TimeDelta, Utc};

use super::{
    Accounting,
    focus_units::{FocusUnit, Presence},
    time_intervals::{Interval, gaps_between, overlaps_with},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusRun {
    pub unit: FocusUnit,
    pub since: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRun {
    pub since: DateTime<Utc>,
}

impl Accounting<'_> {
    fn idle_after(&self) -> TimeDelta {
        TimeDelta::from_std(self.config.idle_after).unwrap_or(TimeDelta::MAX)
    }

    fn unmistakably_away(&self, now: DateTime<Utc>) -> Vec<Interval> {
        self.ledger
            .presence_spans_ended_by(now)
            .filter(|span| matches!(span.state, Presence::Asleep | Presence::ScreenOff))
            .map(|span| span.interval())
            .collect()
    }

    fn breaks(&self, now: DateTime<Utc>) -> Vec<Interval> {
        let idle_after = self.idle_after();
        let away = self.unmistakably_away(now);
        gaps_between(
            &self.ledger.active_intervals(now),
            self.ledger.first_presence_started_at(),
            now,
        )
        .into_iter()
        .filter(|gap| gap.length() >= idle_after || overlaps_with(&away, *gap).next().is_some())
        .collect()
    }

    #[must_use]
    pub fn last_break(&self, now: DateTime<Utc>) -> Option<Interval> {
        self.breaks(now).into_iter().last()
    }

    #[must_use]
    pub fn screen_run(&self, now: DateTime<Utc>) -> Option<ScreenRun> {
        match self.last_break(now) {
            Some(gap) if gap.until >= now => None,
            Some(gap) => Some(ScreenRun { since: gap.until }),
            None => Some(ScreenRun {
                since: self.ledger.first_presence_started_at(),
            }),
        }
    }

    #[must_use]
    pub fn runs(&self, now: DateTime<Utc>) -> Vec<FocusRun> {
        let floor = self
            .last_break(now)
            .map_or(DateTime::<Utc>::MIN_UTC, |gap| gap.until);
        let window_focus_timeline = &self.ledger.window_focus_timeline;
        let Some(focused_window_span) = window_focus_timeline.focused_window_span() else {
            return Vec::new();
        };
        focused_window_span
            .units
            .iter()
            .map(|unit| {
                let chain_start = window_focus_timeline
                    .finished_window_focus_spans()
                    .iter()
                    .rev()
                    .scan(focused_window_span.started_at, |since, span| {
                        (*since <= span.ended_at && span.units.contains(unit)).then(|| {
                            *since = span.started_at;
                            *since
                        })
                    })
                    .last()
                    .unwrap_or(focused_window_span.started_at);
                FocusRun {
                    unit: unit.clone(),
                    since: chain_start.max(floor),
                }
            })
            .collect()
    }

    #[must_use]
    pub fn dominant_run(&self, now: DateTime<Utc>) -> Option<FocusRun> {
        self.runs(now).into_iter().min_by_key(|run| run.since)
    }

    #[must_use]
    pub fn longest_run_within(&self, window: Interval, now: DateTime<Utc>) -> Option<Interval> {
        self.ledger
            .active_within(window, now)
            .into_iter()
            .max_by_key(|run| run.length())
    }

    #[must_use]
    pub fn breaks_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<Interval> {
        self.breaks(now)
            .into_iter()
            .filter_map(|gap| gap.intersect(window))
            .collect()
    }

    pub fn screen_spent(&self, window: Interval, now: DateTime<Utc>) -> TimeDelta {
        self.ledger
            .active_within(window, now)
            .into_iter()
            .map(Interval::length)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::login_session_presence::{
        focus_time_ledger::{
            ScreenEdge,
            test_steps::{
                at, config, focused, folded, idle_edge, kitty, no_window_focused,
                screen_power_edge, sleep_edge, step, whole_day,
            },
        },
        logind_manager::SleepEdge,
        wayland_idle_notify::IdleEdge,
    };

    #[test]
    fn a_short_pause_is_not_spent_but_only_a_long_one_is_a_break() {
        let config = config();
        let paused = folded([
            step(focused("kitty", None), 0),
            step(idle_edge(IdleEdge::Idled), 10),
            step(idle_edge(IdleEdge::Resumed), 11),
        ]);
        let paused = paused.accounted(&config);
        assert_eq!(
            paused.spent(&kitty(), whole_day(), at(20)),
            TimeDelta::minutes(19)
        );
        assert_eq!(paused.screen_run(at(20)), Some(ScreenRun { since: at(0) }));

        let rested = folded([
            step(focused("kitty", None), 0),
            step(idle_edge(IdleEdge::Idled), 20),
            step(idle_edge(IdleEdge::Resumed), 30),
        ]);
        let rested = rested.accounted(&config);
        assert_eq!(
            rested.spent(&kitty(), whole_day(), at(40)),
            TimeDelta::minutes(30)
        );
        assert_eq!(
            rested.last_break(at(40)),
            Some(Interval {
                from: at(20),
                until: at(30)
            })
        );
        assert_eq!(rested.screen_run(at(40)), Some(ScreenRun { since: at(30) }));
        assert_eq!(rested.runs(at(40))[0].since, at(30));
    }

    #[test]
    fn suspending_is_a_break_and_being_away_has_no_screen_run() {
        let config = config();
        let asleep = folded([
            step(focused("kitty", None), 0),
            step(sleep_edge(SleepEdge::Suspending), 5),
        ]);
        assert_eq!(asleep.accounted(&config).screen_run(at(30)), None);
        let woken = asleep.fold(sleep_edge(SleepEdge::Resumed), at(60)).ledger;
        assert_eq!(
            woken.accounted(&config).screen_run(at(61)),
            Some(ScreenRun { since: at(60) })
        );
    }

    #[test]
    fn a_dark_screen_is_a_break_until_it_lights_up_again() {
        let config = config();
        let dark = folded([
            step(focused("kitty", None), 0),
            step(screen_power_edge(ScreenEdge::Off), 10),
            step(idle_edge(IdleEdge::Resumed), 20),
        ]);
        let accounted = dark.accounted(&config);
        assert_eq!(accounted.screen_run(at(11)), None);
        assert_eq!(
            accounted.last_break(at(11)),
            Some(Interval {
                from: at(10),
                until: at(11)
            })
        );
        assert_eq!(accounted.screen_run(at(30)), None);
        assert_eq!(
            accounted.spent(&kitty(), whole_day(), at(30)),
            TimeDelta::minutes(10)
        );
        assert_eq!(dark.screen_off_since(), Some(at(10)));

        let lit = dark.fold(screen_power_edge(ScreenEdge::On), at(40)).ledger;
        assert_eq!(
            lit.accounted(&config).screen_run(at(41)),
            Some(ScreenRun { since: at(40) })
        );
        assert_eq!(lit.screen_off_since(), None);
    }

    #[test]
    fn a_lock_screen_taking_focus_during_suspend_does_not_end_the_sleep() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(screen_power_edge(ScreenEdge::Off), 10),
            step(sleep_edge(SleepEdge::Suspending), 50),
            step(no_window_focused(), 50),
            step(screen_power_edge(ScreenEdge::On), 51),
            step(sleep_edge(SleepEdge::Resumed), 800),
        ]);
        let accounted = ledger.accounted(&config);
        assert_eq!(accounted.screen_run(at(400)), None);
        assert_eq!(
            accounted.screen_run(at(801)),
            Some(ScreenRun { since: at(800) })
        );
    }

    #[test]
    fn focusing_a_window_while_the_screen_is_off_keeps_it_off() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(screen_power_edge(ScreenEdge::Off), 10),
            step(focused("firefox", None), 20),
        ]);
        assert_eq!(ledger.screen_off_since(), Some(at(10)));
        assert_eq!(ledger.accounted(&config).screen_run(at(30)), None);
    }

    #[test]
    fn idling_while_the_screen_is_off_stays_screen_off_and_lighting_it_returns_to_idle() {
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(screen_power_edge(ScreenEdge::Off), 10),
            step(idle_edge(IdleEdge::Idled), 11),
        ]);
        assert_eq!(ledger.screen_off_since(), Some(at(10)));
        let lit = ledger
            .fold(screen_power_edge(ScreenEdge::On), at(20))
            .ledger;
        assert_eq!(lit.screen_off_since(), None);
        assert_eq!(
            lit.presence_spans_ended_by(at(21)).last().unwrap().state,
            Presence::Idle
        );
    }

    #[test]
    fn switching_apps_inside_a_project_keeps_the_project_run() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", Some("/code/sottomano")), 0),
            step(focused("code", Some("/code/sottomano")), 15),
        ]);
        let accounted = ledger.accounted(&config);
        let runs = accounted.runs(at(20));
        let run_of = |unit: FocusUnit| runs.iter().find(|run| run.unit == unit).unwrap().since;
        assert_eq!(
            run_of(FocusUnit::Project(PathBuf::from("/code/sottomano"))),
            at(0)
        );
        assert_eq!(run_of(FocusUnit::App("code".to_owned())), at(15));
        assert_eq!(
            accounted.dominant_run(at(20)).unwrap().unit,
            FocusUnit::Project(PathBuf::from("/code/sottomano"))
        );
    }

    #[test]
    fn breaks_are_clipped_to_the_window() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(idle_edge(IdleEdge::Idled), 10),
            step(idle_edge(IdleEdge::Resumed), 30),
        ]);
        let window = Interval {
            from: at(20),
            until: at(60),
        };
        assert_eq!(
            ledger.accounted(&config).breaks_within(window, at(40)),
            vec![Interval {
                from: at(20),
                until: at(30)
            }]
        );
    }

    #[test]
    fn the_longest_run_ends_at_any_pause() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(idle_edge(IdleEdge::Idled), 10),
            step(idle_edge(IdleEdge::Resumed), 11),
            step(idle_edge(IdleEdge::Idled), 25),
            step(idle_edge(IdleEdge::Resumed), 35),
        ]);
        assert_eq!(
            ledger
                .accounted(&config)
                .longest_run_within(whole_day(), at(40)),
            Some(Interval {
                from: at(11),
                until: at(25)
            })
        );
    }
}
