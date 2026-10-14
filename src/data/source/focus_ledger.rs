mod focus_identity;
mod focus_units;
mod presence_accounting;
mod recorded_spans;
#[cfg(test)]
mod test_steps;
mod time_intervals;
mod unit_accounting;

use std::path::Path;

use chrono::{DateTime, Utc};

use self::recorded_spans::OpenSpan;
pub use self::{
    focus_identity::{FocusIdentity, Focused},
    focus_units::{FocusUnit, Presence, StoredFacet},
    presence_accounting::{FocusRun, ScreenRun},
    recorded_spans::{FocusScope, FocusSpan, LoadedSpans, OpenedSpan, PresenceSpan, SpanWrite},
    time_intervals::{Interval, WakingWindow},
    unit_accounting::UnitTotal,
};
use crate::{
    config::widget_groups::focus_time::FocusTimeConfig,
    data::source::{drm_connectors::ScreenEdge, idle_notify::IdleEdge, logind::SleepEdge},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusSignal {
    Focused(Option<Focused>),
    Idle(IdleEdge),
    Sleep(SleepEdge),
    Screen(ScreenEdge),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusLedger {
    focus: Vec<FocusSpan>,
    open_focus: Option<OpenSpan<FocusSpan>>,
    presence: Vec<PresenceSpan>,
    open_presence: Option<OpenSpan<PresenceSpan>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folded {
    pub ledger: FocusLedger,
    pub writes: Vec<SpanWrite>,
}

impl Folded {
    fn then(self, step: impl FnOnce(FocusLedger) -> Folded) -> Folded {
        let next = step(self.ledger);
        Folded {
            ledger: next.ledger,
            writes: self.writes.into_iter().chain(next.writes).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Accounting<'a> {
    pub ledger: &'a FocusLedger,
    pub config: &'a FocusTimeConfig,
}

impl FocusLedger {
    #[must_use]
    pub fn with_history(self, loaded: LoadedSpans) -> Self {
        Self {
            focus: loaded.focus.into_iter().chain(self.focus).collect(),
            presence: loaded.presence.into_iter().chain(self.presence).collect(),
            ..self
        }
    }

    pub fn accounted<'a>(&'a self, config: &'a FocusTimeConfig) -> Accounting<'a> {
        Accounting {
            ledger: self,
            config,
        }
    }

    pub fn focused_units(&self) -> &[FocusUnit] {
        self.open_focus
            .as_ref()
            .map_or(&[], |open| open.span.units.as_slice())
    }

    pub fn project_shown_in(&self, window: u64) -> Option<&Path> {
        self.open_focus
            .as_ref()
            .filter(|open| open.span.window == window)?
            .span
            .units
            .iter()
            .find_map(|unit| match unit {
                FocusUnit::Project(root) => Some(root.as_path()),
                FocusUnit::App(_) => None,
            })
    }

    pub fn screen_off_since(&self) -> Option<DateTime<Utc>> {
        self.open_presence
            .as_ref()
            .filter(|open| open.span.state == Presence::ScreenOff)
            .map(|open| open.span.started_at)
    }

    pub fn fold(self, signal: FocusSignal, now: DateTime<Utc>) -> Folded {
        match signal {
            FocusSignal::Focused(focused) => self
                .present_as(Presence::Active, now)
                .then(|ledger| ledger.refocused(focused, now)),
            FocusSignal::Idle(IdleEdge::Idled) => self
                .focus_extended(now)
                .then(|ledger| ledger.present_as(Presence::Idle, now)),
            FocusSignal::Sleep(SleepEdge::Suspending) => self
                .focus_extended(now)
                .then(|ledger| ledger.present_as(Presence::Asleep, now)),
            FocusSignal::Screen(ScreenEdge::Off) => self
                .focus_extended(now)
                .then(|ledger| ledger.present_as(Presence::ScreenOff, now)),
            FocusSignal::Idle(IdleEdge::Resumed) if self.screen_off_since().is_some() => {
                self.focus_extended(now)
            }
            FocusSignal::Idle(IdleEdge::Resumed)
            | FocusSignal::Sleep(SleepEdge::Resumed)
            | FocusSignal::Screen(ScreenEdge::On) => self
                .focus_extended(now)
                .then(|ledger| ledger.present_as(Presence::Active, now)),
        }
    }

    #[must_use]
    pub fn with_focus_opened(self, opened: OpenedSpan) -> Self {
        Self {
            open_focus: self
                .open_focus
                .map(|open| open.clone().stored_as(opened, open.span.started_at)),
            ..self
        }
    }

    #[must_use]
    pub fn with_presence_opened(self, opened: OpenedSpan) -> Self {
        Self {
            open_presence: self
                .open_presence
                .map(|open| open.clone().stored_as(opened, open.span.started_at)),
            ..self
        }
    }

    fn focus_extended(self, now: DateTime<Utc>) -> Folded {
        let open_focus = self.open_focus.map(|open| open.ending_at(now));
        Folded {
            writes: open_focus
                .iter()
                .filter_map(OpenSpan::<FocusSpan>::extension)
                .collect(),
            ledger: Self { open_focus, ..self },
        }
    }

    fn refocused(self, focused: Option<Focused>, now: DateTime<Utc>) -> Folded {
        let opened = focused.map(|focused| FocusSpan {
            window: focused.window,
            units: focused.units(),
            started_at: now,
            ended_at: now,
        });
        let extended = self.focus_extended(now);
        let continued = extended
            .ledger
            .open_focus
            .as_ref()
            .zip(opened.as_ref())
            .is_some_and(|(open, opened)| open.span.continues(opened));
        if continued {
            return extended;
        }
        extended.then(|ledger| Folded {
            writes: opened
                .clone()
                .map(SpanWrite::OpenFocus)
                .into_iter()
                .collect(),
            ledger: Self {
                focus: ledger
                    .focus
                    .into_iter()
                    .chain(ledger.open_focus.map(|open| open.span))
                    .collect(),
                open_focus: opened.map(OpenSpan::pending),
                ..ledger
            },
        })
    }

    fn present_as(self, state: Presence, now: DateTime<Utc>) -> Folded {
        let open_presence = self.open_presence.map(|open| open.ending_at(now));
        let extension = open_presence
            .iter()
            .filter_map(OpenSpan::<PresenceSpan>::extension);
        if open_presence
            .as_ref()
            .is_some_and(|open| open.span.state == state)
        {
            return Folded {
                writes: extension.collect(),
                ledger: Self {
                    open_presence,
                    ..self
                },
            };
        }
        let opened = PresenceSpan {
            state,
            started_at: now,
            ended_at: now,
        };
        Folded {
            writes: extension
                .chain([SpanWrite::OpenPresence(opened.clone())])
                .collect(),
            ledger: Self {
                presence: self
                    .presence
                    .into_iter()
                    .chain(open_presence.map(|open| open.span))
                    .collect(),
                open_presence: Some(OpenSpan::pending(opened)),
                ..self
            },
        }
    }

    fn presence_until(&self, now: DateTime<Utc>) -> impl Iterator<Item = PresenceSpan> + '_ {
        self.presence.iter().cloned().chain(
            self.open_presence
                .iter()
                .map(move |open| open.span.clone().ending_at(now)),
        )
    }

    fn focus_until(&self, now: DateTime<Utc>) -> impl Iterator<Item = FocusSpan> + '_ {
        self.focus.iter().cloned().chain(
            self.open_focus
                .iter()
                .map(move |open| open.span.clone().ending_at(now)),
        )
    }

    fn first_presence(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.presence_until(now).map(|span| span.started_at).min()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        recorded_spans::RowId,
        test_steps::{at, config, focused, focused_in, folded, step},
        *,
    };

    #[test]
    fn switching_focus_closes_a_span_and_opens_the_next() {
        let Folded { ledger, writes } =
            FocusLedger::default().fold(focused("kitty", Some("/code/vetrata")), at(0));
        assert!(matches!(writes[0], SpanWrite::OpenPresence(_)));
        assert!(matches!(writes[1], SpanWrite::OpenFocus(_)));
        let ledger = ledger
            .with_focus_opened(OpenedSpan {
                id: 7,
                started_at: at(0),
            })
            .with_presence_opened(OpenedSpan {
                id: 1,
                started_at: at(0),
            });

        let Folded { ledger, writes } = ledger.fold(focused("firefox", None), at(10));
        assert!(writes.contains(&SpanWrite::ExtendFocus {
            id: 7,
            ended_at: at(10)
        }));
        assert_eq!(ledger.focus.len(), 1);
        assert_eq!(ledger.focus[0].ended_at, at(10));
        assert_eq!(
            ledger.focused_units(),
            [FocusUnit::App("firefox".to_owned())]
        );
        let config = config();
        assert_eq!(ledger.accounted(&config).runs(at(12))[0].since, at(10));
    }

    #[test]
    fn a_late_id_only_attaches_to_the_span_it_was_opened_for() {
        let ledger = folded([
            step(focused("kitty", None), 0),
            step(focused("firefox", None), 1),
        ])
        .with_focus_opened(OpenedSpan {
            id: 7,
            started_at: at(0),
        });
        assert_eq!(ledger.open_focus.as_ref().unwrap().row, RowId::Pending);
    }

    #[test]
    fn another_window_of_the_same_app_and_project_opens_a_new_span() {
        let ledger = folded([
            step(focused_in(1, "kitty", Some("/code/vetrata")), 0),
            step(focused_in(1, "kitty", Some("/code/vetrata")), 5),
            step(focused_in(2, "kitty", Some("/code/vetrata")), 10),
        ]);
        assert_eq!(ledger.focus.len(), 1);
        assert_eq!(ledger.focus[0].window, 1);
        assert_eq!(ledger.open_focus.unwrap().span.window, 2);
    }
}
