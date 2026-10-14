use std::{
    borrow::Cow,
    cmp::Reverse,
    fmt,
    iter::successors,
    path::{Path, PathBuf},
    str::FromStr,
};

use chrono::{DateTime, Days, Local, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc};
use niri_ipc::state::EventStreamState;
use procfs::process::Process as Proc;
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    config::widget_groups::focus_time::FocusTimeConfig,
    data::source::{ai::quota::Quota, idle_notify::IdleEdge, logind::SleepEdge},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FocusUnit {
    App(String),
    Project(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFacet {
    pub facet: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownFacet(pub String);

impl fmt::Display for UnknownFacet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown focus facet {}", self.0)
    }
}

impl From<&FocusUnit> for StoredFacet {
    fn from(unit: &FocusUnit) -> Self {
        match unit {
            FocusUnit::App(app_id) => StoredFacet {
                facet: "app".to_owned(),
                value: app_id.clone(),
            },
            FocusUnit::Project(root) => StoredFacet {
                facet: "project".to_owned(),
                value: root.to_string_lossy().into_owned(),
            },
        }
    }
}

impl TryFrom<StoredFacet> for FocusUnit {
    type Error = UnknownFacet;

    fn try_from(stored: StoredFacet) -> Result<Self, UnknownFacet> {
        match stored.facet.as_str() {
            "app" => Ok(FocusUnit::App(stored.value)),
            "project" => Ok(FocusUnit::Project(PathBuf::from(stored.value))),
            _ => Err(UnknownFacet(stored.facet)),
        }
    }
}

impl FocusUnit {
    pub fn label(&self) -> Cow<'_, str> {
        match self {
            FocusUnit::App(app_id) => Cow::Borrowed(app_id),
            FocusUnit::Project(root) => root
                .file_name()
                .map_or_else(|| root.to_string_lossy(), |name| name.to_string_lossy()),
        }
    }

    fn budget(&self, config: &FocusTimeConfig) -> Option<TimeDelta> {
        match self {
            FocusUnit::App(app_id) => config.app_daily_budget_minutes.get(app_id),
            FocusUnit::Project(root) => config
                .project_daily_budget_minutes
                .get(root.to_string_lossy().as_ref()),
        }
        .map(|minutes| TimeDelta::minutes(i64::from(*minutes)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Active,
    Idle,
    Asleep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPresence(pub String);

impl fmt::Display for UnknownPresence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown presence state {}", self.0)
    }
}

impl FromStr for Presence {
    type Err = UnknownPresence;

    fn from_str(stored: &str) -> Result<Self, UnknownPresence> {
        match stored {
            "active" => Ok(Presence::Active),
            "idle" => Ok(Presence::Idle),
            "asleep" => Ok(Presence::Asleep),
            _ => Err(UnknownPresence(stored.to_owned())),
        }
    }
}

impl Presence {
    pub const fn stored_name(self) -> &'static str {
        match self {
            Presence::Active => "active",
            Presence::Idle => "idle",
            Presence::Asleep => "asleep",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub from: DateTime<Utc>,
    pub until: DateTime<Utc>,
}

impl Interval {
    pub fn length(self) -> TimeDelta {
        (self.until - self.from).max(TimeDelta::zero())
    }

    pub fn intersect(self, other: Interval) -> Option<Interval> {
        let overlap = Interval {
            from: self.from.max(other.from),
            until: self.until.min(other.until),
        };
        (overlap.from < overlap.until).then_some(overlap)
    }
}

fn merged(mut intervals: Vec<Interval>) -> Vec<Interval> {
    intervals.sort_by_key(|interval| interval.from);
    intervals.into_iter().fold(Vec::new(), |mut merged, next| {
        match merged.last_mut() {
            Some(last) if next.from <= last.until => last.until = last.until.max(next.until),
            _ => merged.push(next),
        }
        merged
    })
}

struct GapScan {
    cursor: DateTime<Utc>,
    gaps: Vec<Interval>,
}

fn gaps_between(covered: &[Interval], from: DateTime<Utc>, until: DateTime<Utc>) -> Vec<Interval> {
    let scanned = covered.iter().fold(
        GapScan {
            cursor: from,
            gaps: Vec::new(),
        },
        |scan, interval| {
            let gap = (interval.from > scan.cursor).then_some(Interval {
                from: scan.cursor,
                until: interval.from,
            });
            GapScan {
                cursor: scan.cursor.max(interval.until),
                gaps: scan.gaps.into_iter().chain(gap).collect(),
            }
        },
    );
    let trailing = (scanned.cursor < until).then_some(Interval {
        from: scanned.cursor,
        until,
    });
    scanned.gaps.into_iter().chain(trailing).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusSpan {
    pub units: Vec<FocusUnit>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
}

impl FocusSpan {
    fn interval(&self) -> Interval {
        Interval {
            from: self.started_at,
            until: self.ended_at,
        }
    }

    fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self { ended_at, ..self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceSpan {
    pub state: Presence,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
}

impl PresenceSpan {
    fn interval(&self) -> Interval {
        Interval {
            from: self.started_at,
            until: self.ended_at,
        }
    }

    fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self { ended_at, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowId {
    Pending,
    Stored(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSpan<S> {
    pub span: S,
    pub row: RowId,
}

impl<S> OpenSpan<S> {
    fn pending(span: S) -> Self {
        Self {
            span,
            row: RowId::Pending,
        }
    }

    fn stored_id(&self) -> Option<i64> {
        match self.row {
            RowId::Pending => None,
            RowId::Stored(id) => Some(id),
        }
    }

    fn stored_as(self, opened: OpenedSpan, started_at: DateTime<Utc>) -> Self {
        match self.row {
            RowId::Pending if started_at == opened.started_at => Self {
                row: RowId::Stored(opened.id),
                ..self
            },
            _ => self,
        }
    }
}

impl OpenSpan<FocusSpan> {
    fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self {
            span: self.span.ending_at(ended_at),
            ..self
        }
    }

    fn extension(&self) -> Option<SpanWrite> {
        self.stored_id().map(|id| SpanWrite::ExtendFocus {
            id,
            ended_at: self.span.ended_at,
        })
    }
}

impl OpenSpan<PresenceSpan> {
    fn ending_at(self, ended_at: DateTime<Utc>) -> Self {
        Self {
            span: self.span.ending_at(ended_at),
            ..self
        }
    }

    fn extension(&self) -> Option<SpanWrite> {
        self.stored_id().map(|id| SpanWrite::ExtendPresence {
            id,
            ended_at: self.span.ended_at,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusIdentity {
    pub window: u64,
    pub title: Option<String>,
    pub app_id: String,
    pub pid: Option<i32>,
}

impl FocusIdentity {
    pub fn of(niri: &EventStreamState) -> Option<Self> {
        let window = niri
            .windows
            .windows
            .values()
            .find(|window| window.is_focused)?;
        let app_id = window.app_id.clone().filter(|app_id| !app_id.is_empty())?;
        Some(Self {
            window: window.id,
            title: window.title.clone(),
            app_id,
            pid: window.pid,
        })
    }

    pub fn probed(self) -> Focused {
        Focused {
            window: self.window,
            project: self.pid.and_then(probe_project),
            app_id: self.app_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Focused {
    pub window: u64,
    pub app_id: String,
    pub project: Option<PathBuf>,
}

impl Focused {
    fn units(&self) -> Vec<FocusUnit> {
        [Some(FocusUnit::App(self.app_id.clone()))]
            .into_iter()
            .chain([self.project.clone().map(FocusUnit::Project)])
            .flatten()
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusSignal {
    Focused(Option<Focused>),
    Idle(IdleEdge),
    Sleep(SleepEdge),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanWrite {
    OpenFocus(FocusSpan),
    ExtendFocus { id: i64, ended_at: DateTime<Utc> },
    OpenPresence(PresenceSpan),
    ExtendPresence { id: i64, ended_at: DateTime<Utc> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenedSpan {
    pub id: i64,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadedSpans {
    pub focus: Vec<FocusSpan>,
    pub presence: Vec<PresenceSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakingWindow {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl WakingWindow {
    pub fn around(self, now: DateTime<Utc>) -> Interval {
        let local = now.with_timezone(&Local);
        let today = local.date_naive();
        let yesterday = today.checked_sub_days(Days::new(1)).unwrap_or(today);
        let tomorrow = today.checked_add_days(Days::new(1)).unwrap_or(today);
        let (start_day, end_day) = if self.start <= self.end {
            (today, today)
        } else if local.time() < self.end {
            (yesterday, today)
        } else {
            (today, tomorrow)
        };
        Interval {
            from: local_at(start_day.and_time(self.start), now),
            until: local_at(end_day.and_time(self.end), now),
        }
    }
}

fn local_at(naive: NaiveDateTime, fallback: DateTime<Utc>) -> DateTime<Utc> {
    Local
        .from_local_datetime(&naive)
        .earliest()
        .or_else(|| {
            Local
                .from_local_datetime(&(naive + TimeDelta::hours(1)))
                .earliest()
        })
        .map_or(fallback, |at| at.with_timezone(&Utc))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusRun {
    pub unit: FocusUnit,
    pub since: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRun {
    pub since: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitTotal {
    pub unit: FocusUnit,
    pub spent: TimeDelta,
    pub budget: Option<TimeDelta>,
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

impl From<LoadedSpans> for FocusLedger {
    fn from(loaded: LoadedSpans) -> Self {
        Self {
            focus: loaded.focus,
            presence: loaded.presence,
            ..Self::default()
        }
    }
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

    pub fn focused_units(&self) -> &[FocusUnit] {
        self.open_focus
            .as_ref()
            .map_or(&[], |open| open.span.units.as_slice())
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
            FocusSignal::Idle(IdleEdge::Resumed) | FocusSignal::Sleep(SleepEdge::Resumed) => self
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
        let units = focused.map(|focused| focused.units()).unwrap_or_default();
        let extended = self.focus_extended(now);
        let same_units = extended
            .ledger
            .open_focus
            .as_ref()
            .is_some_and(|open| open.span.units == units);
        if same_units {
            return extended;
        }
        extended.then(|ledger| {
            let opened = (!units.is_empty()).then_some(FocusSpan {
                units,
                started_at: now,
                ended_at: now,
            });
            Folded {
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
            }
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

    pub fn accounted<'a>(&'a self, config: &'a FocusTimeConfig) -> Accounting<'a> {
        Accounting {
            ledger: self,
            config,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Accounting<'a> {
    pub ledger: &'a FocusLedger,
    pub config: &'a FocusTimeConfig,
}

impl Accounting<'_> {
    fn idle_after(&self) -> TimeDelta {
        TimeDelta::from_std(self.config.idle_after).unwrap_or(TimeDelta::MAX)
    }

    pub fn active_intervals(&self, now: DateTime<Utc>) -> Vec<Interval> {
        let idle_after = self.idle_after();
        merged(
            self.ledger
                .presence_until(now)
                .filter(|span| match span.state {
                    Presence::Active => true,
                    Presence::Idle => span.interval().length() < idle_after,
                    Presence::Asleep => false,
                })
                .map(|span| span.interval())
                .collect(),
        )
    }

    fn breaks(&self, now: DateTime<Utc>) -> Vec<Interval> {
        let idle_after = self.idle_after();
        self.ledger
            .first_presence(now)
            .map(|first| gaps_between(&self.active_intervals(now), first, now))
            .unwrap_or_default()
            .into_iter()
            .filter(|gap| gap.length() >= idle_after)
            .collect()
    }

    pub fn last_break(&self, now: DateTime<Utc>) -> Option<Interval> {
        self.breaks(now).into_iter().last()
    }

    pub fn screen_run(&self, now: DateTime<Utc>) -> Option<ScreenRun> {
        let first = self.ledger.first_presence(now)?;
        match self.last_break(now) {
            Some(gap) if gap.until >= now => None,
            Some(gap) => Some(ScreenRun { since: gap.until }),
            None => Some(ScreenRun { since: first }),
        }
    }

    pub fn runs(&self, now: DateTime<Utc>) -> Vec<FocusRun> {
        let floor = self
            .last_break(now)
            .map_or(DateTime::<Utc>::MIN_UTC, |gap| gap.until);
        let Some(OpenSpan { span: open, .. }) = &self.ledger.open_focus else {
            return Vec::new();
        };
        open.units
            .iter()
            .map(|unit| {
                let chain_start = self
                    .ledger
                    .focus
                    .iter()
                    .rev()
                    .scan(open.started_at, |since, span| {
                        (*since <= span.ended_at && span.units.contains(unit)).then(|| {
                            *since = span.started_at;
                            *since
                        })
                    })
                    .last()
                    .unwrap_or(open.started_at);
                FocusRun {
                    unit: unit.clone(),
                    since: chain_start.max(floor),
                }
            })
            .collect()
    }

    pub fn dominant_run(&self, now: DateTime<Utc>) -> Option<FocusRun> {
        self.runs(now).into_iter().min_by_key(|run| run.since)
    }

    fn active_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<Interval> {
        self.active_intervals(now)
            .into_iter()
            .filter_map(|interval| interval.intersect(window))
            .collect()
    }

    pub fn screen_spent(&self, window: Interval, now: DateTime<Utc>) -> TimeDelta {
        self.active_within(window, now)
            .into_iter()
            .map(Interval::length)
            .sum()
    }

    pub fn spent(&self, unit: &FocusUnit, window: Interval, now: DateTime<Utc>) -> TimeDelta {
        let active = self.active_within(window, now);
        self.ledger
            .focus_until(now)
            .filter(|span| span.units.contains(unit))
            .flat_map(|span| {
                active
                    .iter()
                    .filter_map(move |interval| interval.intersect(span.interval()))
                    .collect::<Vec<_>>()
            })
            .map(Interval::length)
            .sum()
    }

    pub fn unit_totals(&self, window: Interval, now: DateTime<Utc>) -> Vec<UnitTotal> {
        let mut units: Vec<FocusUnit> = self
            .ledger
            .focus_until(now)
            .filter(|span| span.interval().intersect(window).is_some())
            .flat_map(|span| span.units)
            .collect();
        units.sort();
        units.dedup();
        let mut totals: Vec<UnitTotal> = units
            .into_iter()
            .map(|unit| UnitTotal {
                spent: self.spent(&unit, window, now),
                budget: unit.budget(self.config),
                unit,
            })
            .collect();
        totals.sort_by_key(|total| Reverse(total.spent));
        totals
    }

    pub fn screen_budget(&self) -> Option<TimeDelta> {
        self.config
            .screen_daily_budget_minutes
            .map(|minutes| TimeDelta::minutes(i64::from(minutes)))
    }

    pub fn quotas_within(&self, window: Interval, now: DateTime<Utc>) -> Vec<Quota> {
        let quota = |label: Cow<'static, str>, spent: TimeDelta, budget: TimeDelta| {
            let used = spent.as_seconds_f64() / budget.as_seconds_f64().max(1.0);
            Quota::within_window(
                label,
                Ratio::new::<ratio>(used),
                window.until,
                window.length(),
                now,
            )
        };
        let screen = self.screen_budget().map(|budget| {
            quota(
                Cow::Borrowed("Screen"),
                self.screen_spent(window, now),
                budget,
            )
        });
        self.unit_totals(window, now)
            .into_iter()
            .filter_map(|total| {
                let budget = total.budget?;
                Some(quota(
                    Cow::Owned(total.unit.label().into_owned()),
                    total.spent,
                    budget,
                ))
            })
            .chain(screen)
            .collect()
    }
}

pub fn probe_project(pid: i32) -> Option<PathBuf> {
    let leaf = successors(Proc::new(pid).ok(), |process| {
        Proc::new(only_child(process)?).ok()
    })
    .last()?;
    repository_root(&leaf.cwd().ok()?)
}

fn only_child(process: &Proc) -> Option<i32> {
    let children = process.task_main_thread().ok()?.children().ok()?;
    match children.as_slice() {
        [child] => i32::try_from(*child).ok(),
        _ => None,
    }
}

fn repository_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|dir| dir.join(".git").exists() || dir.join(".jj").exists())
        .map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn config() -> FocusTimeConfig {
        FocusTimeConfig {
            idle_after: Duration::from_mins(3),
            ..FocusTimeConfig::default()
        }
    }

    fn focused(app_id: &str, project: Option<&str>) -> FocusSignal {
        FocusSignal::Focused(Some(Focused {
            window: 1,
            app_id: app_id.to_owned(),
            project: project.map(PathBuf::from),
        }))
    }

    struct Step {
        signal: FocusSignal,
        minute: i64,
    }

    fn step(signal: FocusSignal, minute: i64) -> Step {
        Step { signal, minute }
    }

    fn folded(steps: impl IntoIterator<Item = Step>) -> FocusLedger {
        steps
            .into_iter()
            .fold(FocusLedger::default(), |ledger, step| {
                ledger.fold(step.signal, at(step.minute)).ledger
            })
    }

    fn window() -> Interval {
        Interval {
            from: at(-60),
            until: at(600),
        }
    }

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
    fn a_short_pause_counts_as_active_and_a_long_one_is_a_break() {
        let config = config();
        let unit = FocusUnit::App("kitty".to_owned());
        let paused = folded([
            step(focused("kitty", None), 0),
            step(FocusSignal::Idle(IdleEdge::Idled), 10),
            step(FocusSignal::Idle(IdleEdge::Resumed), 11),
        ]);
        let paused = paused.accounted(&config);
        assert_eq!(
            paused.spent(&unit, window(), at(20)),
            TimeDelta::minutes(20)
        );
        assert_eq!(paused.screen_run(at(20)), Some(ScreenRun { since: at(0) }));

        let rested = folded([
            step(focused("kitty", None), 0),
            step(FocusSignal::Idle(IdleEdge::Idled), 20),
            step(FocusSignal::Idle(IdleEdge::Resumed), 30),
        ]);
        let rested = rested.accounted(&config);
        assert_eq!(
            rested.spent(&unit, window(), at(40)),
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
            step(FocusSignal::Sleep(SleepEdge::Suspending), 5),
        ]);
        assert_eq!(asleep.accounted(&config).screen_run(at(30)), None);
        let woken = asleep
            .fold(FocusSignal::Sleep(SleepEdge::Resumed), at(60))
            .ledger;
        assert_eq!(
            woken.accounted(&config).screen_run(at(61)),
            Some(ScreenRun { since: at(60) })
        );
    }

    #[test]
    fn switching_apps_inside_a_project_keeps_the_project_run() {
        let config = config();
        let ledger = folded([
            step(focused("kitty", Some("/code/vetrata")), 0),
            step(focused("code", Some("/code/vetrata")), 15),
        ]);
        let accounted = ledger.accounted(&config);
        let runs = accounted.runs(at(20));
        let run_of = |unit: FocusUnit| runs.iter().find(|run| run.unit == unit).unwrap().since;
        assert_eq!(
            run_of(FocusUnit::Project(PathBuf::from("/code/vetrata"))),
            at(0)
        );
        assert_eq!(run_of(FocusUnit::App("code".to_owned())), at(15));
        assert_eq!(
            accounted.dominant_run(at(20)).unwrap().unit,
            FocusUnit::Project(PathBuf::from("/code/vetrata"))
        );
    }

    #[test]
    fn spent_time_is_clipped_to_the_window() {
        let config = config();
        let ledger = folded([step(focused("kitty", None), 0)]);
        let accounted = ledger.accounted(&config);
        let window = Interval {
            from: at(10),
            until: at(25),
        };
        let unit = FocusUnit::App("kitty".to_owned());
        assert_eq!(
            accounted.spent(&unit, window, at(40)),
            TimeDelta::minutes(15)
        );
        assert_eq!(
            accounted.screen_spent(window, at(40)),
            TimeDelta::minutes(15)
        );
    }

    #[test]
    fn a_budget_outpaced_before_the_window_ends_warns() {
        use crate::data::severity::Severity;
        let mut config = config();
        config
            .app_daily_budget_minutes
            .insert("kitty".to_owned(), 60);
        let ledger = folded([step(focused("kitty", None), 0)]);
        let accounted = ledger.accounted(&config);
        let window = Interval {
            from: at(0),
            until: at(600),
        };
        let quotas = accounted.quotas_within(window, at(120));
        assert_eq!(quotas.len(), 1);
        assert_eq!(quotas[0].severity(), Severity::Critical);
        assert!(accounted.quotas_within(window, at(-30)).is_empty());
    }

    #[test]
    fn overnight_waking_windows_wrap_around_midnight() {
        let waking = WakingWindow {
            start: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(2, 0, 0).unwrap(),
        };
        let now = Local
            .with_ymd_and_hms(2026, 3, 10, 1, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        let window = waking.around(now);
        assert_eq!(
            window.from.with_timezone(&Local).naive_local(),
            "2026-03-09T10:00:00".parse().unwrap()
        );
        assert_eq!(
            window.until.with_timezone(&Local).naive_local(),
            "2026-03-10T02:00:00".parse().unwrap()
        );
    }

    #[test]
    fn stored_facets_round_trip() {
        let unit = FocusUnit::Project(PathBuf::from("/code/vetrata"));
        assert_eq!(FocusUnit::try_from(StoredFacet::from(&unit)), Ok(unit));
        assert!(
            FocusUnit::try_from(StoredFacet {
                facet: "branch".to_owned(),
                value: "main".to_owned()
            })
            .is_err()
        );
        assert_eq!("asleep".parse(), Ok(Presence::Asleep));
    }
}
