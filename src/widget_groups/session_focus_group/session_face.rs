use chrono::{DateTime, Locale, TimeDelta, Utc};

use crate::{
    data::{
        severity::Severity,
        source::focus_ledger::{
            Accounting, FocusRun, FocusUnit, Interval, ScreenRun, WakingWindow,
        },
    },
    widget_groups::session_focus_group::BreakPledge,
    widgets::{
        amount_text::{coarse_duration_text, duration_text},
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        localized_time::LocalizedTimeFormat,
        ongoing_toast::OngoingBody,
        phosphor::Phosphor,
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct SessionFace<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
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

    fn dominant(&self, now: DateTime<Utc>) -> Option<FocusRun> {
        self.accounting.dominant_run(now)
    }

    fn thresholds(&self) -> Section {
        let config = self.accounting.config;
        Section::dictionary([
            KeyValue::new(
                "Warns after",
                humantime::format_duration(config.session_warning).to_string(),
            ),
            KeyValue::new(
                "Critical after",
                humantime::format_duration(config.session_critical).to_string(),
            ),
            KeyValue::new(
                "Break grace",
                humantime::format_duration(config.break_grace).to_string(),
            ),
        ])
        .titled("Thresholds")
    }

    pub fn sections(&self) -> [Section; 3] {
        [self.now(), self.today(), self.thresholds()]
    }

    fn today(&self) -> Section {
        let now = Utc::now();
        let window = self.waking.around(now);
        let breaks = self.accounting.breaks_within(window, now);
        let longest_break = breaks.iter().copied().max_by_key(|gap| gap.length());
        let longest_run = self.accounting.longest_run_within(window, now);
        let entries = [
            Some(KeyValue::new("Breaks", breaks.len().to_string())),
            longest_break.map(|gap| KeyValue::new("Longest break", duration_text(gap.length()))),
            longest_run.map(|run| {
                KeyValue::new(
                    "Longest run",
                    format!(
                        "{}, from {} to {}",
                        duration_text(run.length()),
                        run.from.with_timezone(&chrono::Local).format("%H:%M"),
                        run.until.with_timezone(&chrono::Local).format("%H:%M")
                    ),
                )
            }),
            Some(KeyValue::new(
                "Break after idling",
                humantime::format_duration(self.accounting.config.idle_after).to_string(),
            )),
        ];
        Section::dictionary(entries.into_iter().flatten()).titled("Today")
    }

    fn now(&self) -> Section {
        let now = Utc::now();
        let units = self.accounting.ledger.focused_units();
        let project = units.iter().find_map(|unit| match unit {
            FocusUnit::Project(root) => Some(root.clone()),
            FocusUnit::App(_) => None,
        });
        let app = units.iter().find_map(|unit| match unit {
            FocusUnit::App(app_id) => Some(app_id.clone()),
            FocusUnit::Project(_) => None,
        });
        let entries = [
            project.map(|root| KeyValue::new("Project", Cell::Path(root))),
            app.map(|app_id| KeyValue::new("App", app_id)),
            self.dominant(now).map(|run| {
                KeyValue::new(
                    format!("{} since", run.unit.label()),
                    Cell::Time {
                        at: run.since,
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                )
            }),
            self.accounting.screen_run(now).map(|ScreenRun { since }| {
                KeyValue::new("At the screen for", duration_text(now - since))
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
            self.accounting
                .last_break(now)
                .map(|Interval { from, until }| {
                    KeyValue::new(
                        "Last break",
                        format!(
                            "{} ago, {}",
                            duration_text(now - until),
                            duration_text(until - from)
                        ),
                    )
                }),
        ];
        Section::dictionary(entries.into_iter().flatten()).titled("Now")
    }
}

impl BarWidget for SessionFace<'_> {
    fn name(&self) -> &'static str {
        "Focus session"
    }

    fn icon(&self) -> Glyph {
        Phosphor::UserFocus(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        self.run_length(Utc::now()).map(coarse_duration_text)
    }

    fn state(&self) -> WidgetState {
        self.severity(Utc::now()).into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Warning,
            summary_text: "Long focus session",
        }
    }

    fn ongoing_toast_body(&self, locale: Locale) -> OngoingBody {
        let now = Utc::now();
        let Some(ScreenRun { since }) = self.accounting.screen_run(now) else {
            return OngoingBody::None;
        };
        let mostly = self
            .dominant(now)
            .map(|run| {
                format!(
                    " Mostly {} ({}).",
                    run.unit.label(),
                    duration_text(now - run.since)
                )
            })
            .unwrap_or_default();
        OngoingBody::Text(format!(
            "At the screen since {}, {} without a break.{mostly}",
            LocalizedTimeFormat::TimeOfDay.format(&since, locale),
            duration_text(now - since)
        ))
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        config::widget_groups::focus_time::FocusTimeConfig,
        data::source::{
            focus_ledger::{AwayReasonEdge, FocusLedger, FocusSignal, Focused, WindowFocusChange},
            idle_notify::IdleEdge,
        },
    };

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn all_day() -> WakingWindow {
        WakingWindow {
            start: chrono::NaiveTime::MIN,
            end: chrono::NaiveTime::MIN,
        }
    }

    fn focused() -> FocusSignal {
        FocusSignal::WindowFocusChanged(WindowFocusChange::WindowGainedFocus(Focused {
            window: 1,
            app_id: "kitty".to_owned(),
            project: None,
        }))
    }

    fn idle_edge(edge: IdleEdge) -> FocusSignal {
        FocusSignal::AwayReasonChanged(AwayReasonEdge::IdleNotifierEdge(edge))
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
        let ledger = FocusLedger::started_at(at(0)).fold(focused(), at(0)).ledger;
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            waking: all_day(),
            pledge: Some(BreakPledge { at: at(10) }),
        };
        assert_eq!(face.severity(at(12)), Severity::Normal);
        assert_eq!(face.severity(at(16)), Severity::Critical);
    }

    #[test]
    fn a_break_after_the_pledge_honours_it() {
        let config = config();
        let ledger = [
            (focused(), 0),
            (idle_edge(IdleEdge::Idled), 11),
            (idle_edge(IdleEdge::Resumed), 15),
        ]
        .into_iter()
        .fold(
            FocusLedger::started_at(at(0)),
            |ledger, (signal, minute)| ledger.fold(signal, at(minute)).ledger,
        );
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            waking: all_day(),
            pledge: Some(BreakPledge { at: at(10) }),
        };
        assert_eq!(face.severity(at(30)), Severity::Normal);
    }
}
