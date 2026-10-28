use chrono::{DateTime, Locale, TimeDelta, Utc};
use quick_xml::escape::escape;
use sottomano_config::severity::Severity;
use sottomano_sources::login_session_presence::focus_time_ledger::{
    Accounting, FocusUnit, Interval, ScreenRun, UnitTotal,
};

use crate::{
    clock::Clock,
    widget_groups::session_focus_group::BreakPledge,
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{definition_list, moment_text},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct SessionFace<'a> {
    pub accounting: Accounting<'a>,
    pub pledge: Option<BreakPledge>,
}

impl SessionFace<'_> {
    fn pledge_broken(&self, now: DateTime<Utc>) -> bool {
        let Some(pledge) = self.pledge else {
            return false;
        };
        let grace =
            TimeDelta::from_std(self.accounting.config.break_grace).unwrap_or(TimeDelta::MAX);
        let honoured = self
            .accounting
            .last_break(now)
            .is_some_and(|gap| gap.until >= pledge.at)
            || self.accounting.screen_run(now).is_none();
        !honoured && now - pledge.at >= grace
    }

    fn run_length(&self, now: DateTime<Utc>) -> Option<TimeDelta> {
        self.accounting
            .screen_run(now)
            .map(|ScreenRun { since }| now - since)
    }

    fn severity(&self, now: DateTime<Utc>) -> Severity {
        let config = self.accounting.config;
        let critical = TimeDelta::from_std(config.session_critical).unwrap_or(TimeDelta::MAX);
        let warning = TimeDelta::from_std(config.session_warning).unwrap_or(TimeDelta::MAX);
        match self.run_length(now) {
            _ if self.pledge_broken(now) => Severity::Critical,
            Some(run) if run >= critical => Severity::Critical,
            Some(run) if run >= warning => Severity::Warning,
            _ => Severity::Normal,
        }
    }

    fn screen_run_summary(&self, now: DateTime<Utc>, locale: Locale) -> Option<String> {
        let ScreenRun { since } = self.accounting.screen_run(now)?;
        let totals = self.accounting.unit_totals(
            Interval {
                from: since,
                until: now,
            },
            now,
        );
        let most_used_project = totals.iter().find(|total| match total.unit {
            FocusUnit::Project(_) => true,
            FocusUnit::App(_) => false,
        });
        let most_used_app = totals.iter().find(|total| match total.unit {
            FocusUnit::App(_) => true,
            FocusUnit::Project(_) => false,
        });
        let unit_row = |key, total: &UnitTotal| {
            (
                key,
                format!(
                    "{} · {}",
                    escape(total.unit.label().as_ref()),
                    duration_text(total.spent, locale)
                ),
            )
        };
        Some(definition_list(
            [
                ("Active since", moment_text(since, now, locale)),
                (
                    "Active since last break",
                    duration_text(now - since, locale),
                ),
            ]
            .into_iter()
            .chain(most_used_project.map(|total| unit_row("Project", total)))
            .chain(most_used_app.map(|total| unit_row("App", total))),
        ))
    }

    fn session_alerts(&self) -> Section {
        let config = self.accounting.config;
        Section::dictionary([
            KeyValue::new(
                "Warning after active for",
                Cell::duration(config.session_warning),
            ),
            KeyValue::new(
                "Critical after active for",
                Cell::duration(config.session_critical),
            ),
            KeyValue::new(
                "Break must start within",
                Cell::duration(config.break_grace),
            ),
        ])
        .titled((
            "Session alerts".to_owned(),
            "Alerts follow 'Active since last break'. After you press 'Taking a break', a break \
             must start within the time shown or the session turns critical."
                .to_owned(),
        ))
    }

    pub fn sections(&self, now: DateTime<Utc>) -> [Section; 2] {
        [self.current_session(now), self.session_alerts()]
    }

    fn current_session(&self, now: DateTime<Utc>) -> Section {
        let last_break = self.accounting.last_break(now);
        let entries = [
            self.accounting.screen_run(now).map(|ScreenRun { since }| {
                KeyValue::new("Active since last break", Cell::duration(now - since))
                    .level(self.severity(now))
            }),
            self.accounting.ledger.screen_off_since().map(|since| {
                KeyValue::new(
                    "Screen off since",
                    Cell::Time {
                        at: since,
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                )
            }),
            last_break.map(|Interval { until, .. }| {
                KeyValue::new("Last break ended", Cell::RelativeMoment(until))
            }),
            last_break.map(|Interval { from, until }| {
                KeyValue::new("Last break lasted", Cell::duration(until - from))
            }),
        ];
        Section::dictionary(entries.into_iter().flatten()).titled((
            "Current session".to_owned(),
            "Active means recent keyboard or pointer input. The count starts when the last break \
             ended; the Breaks reading shows what counts as one."
                .to_owned(),
        ))
    }
}

impl BarWidget for SessionFace<'_> {
    fn name(&self) -> &'static str {
        "Focus session"
    }

    fn icon(&self) -> Glyph {
        Phosphor::UserFocus(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.run_length(clock.utc())
            .map(|run_length| TimeDelta::minutes(run_length.num_minutes()))
            .filter(|whole_minutes| !whole_minutes.is_zero())
            .map(|whole_minutes| duration_text(whole_minutes, locale))
    }

    fn state(&self, clock: Clock) -> WidgetState {
        self.severity(clock.utc()).into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "Long focus session",
        }
    }

    fn ongoing_toast_body(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.screen_run_summary(clock.utc(), locale)
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        TooltipBody::sections(self.sections(clock.utc()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_config::widget_groups::focus_time::FocusTimeConfig;
    use sottomano_sources::login_session_presence::{
        focus_time_ledger::{FocusLedger, FocusSignal, Focused},
        wayland_idle_notify::{IdleEdge, InputIdleEdge},
    };

    use super::*;

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn focused(window: u64, app_id: &str) -> FocusSignal {
        FocusSignal::WindowFocused(Focused {
            window,
            app_id: app_id.to_owned(),
            project: None,
        })
    }

    fn ledger_after(signals: impl IntoIterator<Item = (FocusSignal, i64)>) -> FocusLedger {
        let mut ledger = FocusLedger::started_at(at(0));
        for (signal, minute) in signals {
            ledger.fold(signal, at(minute), &config());
        }
        ledger
    }

    fn config() -> FocusTimeConfig {
        FocusTimeConfig {
            idle_after: Duration::from_mins(3),
            break_grace: Duration::from_mins(5),
            ..FocusTimeConfig::default()
        }
    }

    #[test]
    fn a_pledge_without_a_break_turns_critical_after_the_grace() {
        let config = config();
        let ledger = ledger_after([(focused(1, "kitty"), 0)]);
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            pledge: Some(BreakPledge { at: at(10) }),
        };
        assert_eq!(face.severity(at(12)), Severity::Normal);
        assert_eq!(face.severity(at(16)), Severity::Critical);
    }

    #[test]
    fn a_break_after_the_pledge_honours_it() {
        let config = config();
        let ledger = ledger_after([
            (focused(1, "kitty"), 0),
            (
                FocusSignal::Idle(InputIdleEdge::simulated(IdleEdge::Idled)),
                11,
            ),
            (
                FocusSignal::Idle(InputIdleEdge::simulated(IdleEdge::Resumed)),
                15,
            ),
        ]);
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            pledge: Some(BreakPledge { at: at(10) }),
        };
        assert_eq!(face.severity(at(30)), Severity::Normal);
    }

    #[test]
    fn the_summary_names_the_most_used_app_not_the_latest_focus() {
        let config = config();
        let ledger = ledger_after([(focused(1, "firefox"), 0), (focused(2, "kitty"), 40)]);
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            pledge: None,
        };
        let summary = face.screen_run_summary(at(41), Locale::en_US).unwrap();
        assert!(summary.contains("<dt>App</dt><dd>firefox · 40m</dd>"));
        assert!(!summary.contains("kitty"));
    }
}
