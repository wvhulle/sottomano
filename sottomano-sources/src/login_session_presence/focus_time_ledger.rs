mod away_reasons;
mod focus_identity;
mod focus_units;
mod presence_accounting;
mod presence_timeline;
mod recorded_spans;
mod switch_heat_gradient;
#[cfg(test)]
mod test_steps;
mod time_intervals;
mod unit_accounting;
mod window_focus_timeline;

use std::{mem::take, path::Path};

use chrono::{DateTime, TimeDelta, Utc};
use sottomano_config::widget_groups::focus_time::FocusTimeConfig;

use self::{away_reasons::LatestAwayReasons, window_focus_timeline::OngoingWindowFocus};
pub use self::{
    away_reasons::ScreenEdge,
    focus_identity::{FocusIdentity, Focused},
    focus_units::{FocusUnit, Presence, StoredFacet},
    presence_accounting::{BreakCause, FocusRun, ScreenRun, TakenBreak},
    recorded_spans::{FocusSpan, LoadedSpans, PresenceSpan, SpanWrite},
    time_intervals::{Interval, LAST_DAY, LAST_HOUR, TrailingPeriod},
    unit_accounting::{BudgetStatus, UnitTotal},
};
use crate::login_session_presence::{logind_manager::SleepEdge, wayland_idle_notify::IdleEdge};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FocusSignal {
    WindowFocused(Focused),
    #[default]
    NoWindowFocused,
    Idle(IdleEdge),
    Sleep(SleepEdge),
    ScreenPower(ScreenEdge),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusLedger {
    tracking_started_at: DateTime<Utc>,
    finished_window_focus_spans: Vec<FocusSpan>,
    ongoing_window_focus: OngoingWindowFocus,
    finished_presence_spans: Vec<PresenceSpan>,
    ongoing_presence_span: PresenceSpan,
    latest_away_reasons: LatestAwayReasons,
}

#[derive(Debug, Clone, Copy)]
pub struct Accounting<'a> {
    pub ledger: &'a FocusLedger,
    pub config: &'a FocusTimeConfig,
}

impl FocusLedger {
    #[must_use]
    pub fn started_at(now: DateTime<Utc>) -> Self {
        Self {
            tracking_started_at: now,
            finished_window_focus_spans: Vec::new(),
            ongoing_window_focus: OngoingWindowFocus::NoWindowFocused,
            finished_presence_spans: Vec::new(),
            ongoing_presence_span: PresenceSpan {
                state: Presence::Active,
                started_at: now,
                ended_at: now,
            },
            latest_away_reasons: LatestAwayReasons::default(),
        }
    }

    pub fn load_history(&mut self, loaded: LoadedSpans, config: &FocusTimeConfig) {
        let tracking_started_at = self.tracking_started_at;
        let shortest_counted_focus =
            TimeDelta::from_std(config.shortest_counted_focus).unwrap_or(TimeDelta::MAX);
        let tracked_spans = take(&mut self.finished_window_focus_spans);
        for span in loaded
            .window_focus_spans
            .into_iter()
            .filter(|span| span.started_at < tracking_started_at)
            .chain(tracked_spans)
        {
            self.keep_finished_window_focus(span, shortest_counted_focus);
        }
        self.finished_presence_spans = loaded
            .presence_spans
            .into_iter()
            .filter(|span| span.started_at < tracking_started_at)
            .chain(self.finished_presence_spans.drain(..))
            .collect();
    }

    #[must_use]
    pub fn accounted<'a>(&'a self, config: &'a FocusTimeConfig) -> Accounting<'a> {
        Accounting {
            ledger: self,
            config,
        }
    }

    pub fn fold(
        &mut self,
        signal: FocusSignal,
        now: DateTime<Utc>,
        config: &FocusTimeConfig,
    ) -> Vec<SpanWrite> {
        let shortest_counted_focus =
            TimeDelta::from_std(config.shortest_counted_focus).unwrap_or(TimeDelta::MAX);
        self.ongoing_presence_span.ended_at = now;
        let mut span_writes: Vec<SpanWrite> = self
            .ongoing_focus_write(now, shortest_counted_focus)
            .into_iter()
            .chain([SpanWrite::RecordPresenceSpan(
                self.ongoing_presence_span.clone(),
            )])
            .collect();
        match signal {
            FocusSignal::WindowFocused(focused) => {
                self.focus_window(&focused, now, shortest_counted_focus);
            }
            FocusSignal::NoWindowFocused => self.lose_window_focus(shortest_counted_focus),
            away_signal => span_writes.extend(self.follow_away_reason(&away_signal, now)),
        }
        span_writes
    }

    #[must_use]
    pub fn project_shown_in(&self, window: u64) -> Option<&Path> {
        self.focused_window_span()
            .filter(|span| span.window == window)?
            .units
            .iter()
            .find_map(|unit| match unit {
                FocusUnit::Project(root) => Some(root.as_path()),
                FocusUnit::App(_) => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        test_steps::{at, config, focused, focused_in, folded, no_window_focused, step, whole_day},
        *,
    };

    fn recorded_window_focus_spans(span_writes: &[SpanWrite]) -> Vec<&FocusSpan> {
        span_writes
            .iter()
            .filter_map(|write| match write {
                SpanWrite::RecordWindowFocusSpan(span) => Some(span),
                SpanWrite::RecordPresenceSpan(_) => None,
            })
            .collect()
    }

    #[test]
    fn switching_focus_finishes_a_span_and_records_it() {
        let config = config();
        let mut ledger = FocusLedger::started_at(at(0));
        let span_writes = ledger.fold(focused("kitty", Some("/code/sottomano")), at(0), &config);
        assert!(recorded_window_focus_spans(&span_writes).is_empty());

        let span_writes = ledger.fold(focused("firefox", None), at(10), &config);
        let recorded = recorded_window_focus_spans(&span_writes);
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].started_at, at(0));
        assert_eq!(recorded[0].ended_at, at(10));
        assert_eq!(ledger.finished_window_focus_spans.len(), 1);
        assert_eq!(ledger.finished_window_focus_spans[0].ended_at, at(10));
        assert_eq!(
            ledger.focused_units(),
            [FocusUnit::App("firefox".to_owned())]
        );
        assert_eq!(ledger.accounted(&config).runs(at(12))[0].since, at(10));
    }

    #[test]
    fn a_focus_flicker_is_neither_kept_nor_recorded_nor_splits_the_stay() {
        let config = config();
        let flicker_started_at = at(10);
        let flicker_ended_at = flicker_started_at + TimeDelta::milliseconds(400);
        let mut ledger = FocusLedger::started_at(at(0));
        ledger.fold(focused_in(1, "kitty", None), at(0), &config);
        ledger.fold(focused_in(2, "firefox", None), flicker_started_at, &config);
        let span_writes = ledger.fold(focused_in(1, "kitty", None), flicker_ended_at, &config);
        assert!(recorded_window_focus_spans(&span_writes).is_empty());
        assert!(ledger.finished_window_focus_spans.is_empty());
        assert_eq!(ledger.focused_window_span().unwrap().started_at, at(0));
        assert_eq!(
            ledger
                .accounted(&config)
                .switches_within(whole_day(), at(20)),
            1
        );
    }

    #[test]
    fn loaded_flickers_are_dropped_and_the_stays_around_them_joined() {
        let config = config();
        let kitty_span = |started_at, ended_at| FocusSpan {
            window: 1,
            units: vec![FocusUnit::App("kitty".to_owned())],
            started_at,
            ended_at,
        };
        let flicker_ended_at = at(-20) + TimeDelta::milliseconds(300);
        let mut ledger = FocusLedger::started_at(at(0));
        ledger.load_history(
            LoadedSpans {
                window_focus_spans: vec![
                    kitty_span(at(-30), at(-20)),
                    FocusSpan {
                        window: 2,
                        ..kitty_span(at(-20), flicker_ended_at)
                    },
                    kitty_span(flicker_ended_at, at(-5)),
                ],
                presence_spans: Vec::new(),
            },
            &config,
        );
        assert_eq!(
            ledger.finished_window_focus_spans,
            vec![kitty_span(at(-30), at(-5))]
        );
    }

    #[test]
    fn losing_window_focus_finishes_the_span_without_presence_changing() {
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(no_window_focused(), 10),
        ]);
        assert!(ledger.focused_units().is_empty());
        assert_eq!(ledger.finished_window_focus_spans[0].ended_at, at(10));
        let config = config();
        assert_eq!(
            ledger.accounted(&config).screen_run(at(20)),
            Some(ScreenRun { since: at(0) })
        );
    }

    #[test]
    fn another_window_of_the_same_app_and_project_opens_a_new_span() {
        let ledger = folded([
            step(focused_in(1, "kitty", Some("/code/sottomano")), 0),
            step(focused_in(1, "kitty", Some("/code/sottomano")), 5),
            step(focused_in(2, "kitty", Some("/code/sottomano")), 10),
        ]);
        assert_eq!(ledger.finished_window_focus_spans.len(), 1);
        assert_eq!(ledger.finished_window_focus_spans[0].window, 1);
        assert_eq!(ledger.focused_window_span().unwrap().window, 2);
    }

    #[test]
    fn loaded_history_keeps_only_spans_from_before_tracking_started() {
        let mut ledger = folded([
            step(focused("kitty", None), 0),
            step(focused("firefox", None), 10),
        ]);
        let earlier_span = FocusSpan {
            window: 9,
            units: vec![FocusUnit::App("mpv".to_owned())],
            started_at: at(-30),
            ended_at: at(-20),
        };
        let duplicated_span = ledger.finished_window_focus_spans[0].clone();
        ledger.load_history(
            LoadedSpans {
                window_focus_spans: vec![earlier_span.clone(), duplicated_span.clone()],
                presence_spans: Vec::new(),
            },
            &config(),
        );
        assert_eq!(
            ledger.finished_window_focus_spans,
            vec![earlier_span, duplicated_span]
        );
    }
}
