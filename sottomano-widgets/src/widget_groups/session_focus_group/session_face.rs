use chrono::{DateTime, Locale, TimeDelta, Utc};
use quick_xml::escape::escape;
use sottomano_config::severity::Severity;
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, FocusUnit, Interval, ScreenRun, UnitTotal,
};

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widget_groups::session_focus_group::WarningSnooze,
    widgets::{
        icon_glyph::IconGlyph,
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{definition_list, moment_text},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct SessionFace<'a> {
    pub accounting: Accounting<'a>,
    pub snooze: Option<WarningSnooze>,
}

impl SessionFace<'_> {
    fn snoozed_until(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.snooze
            .map(|WarningSnooze { until }| until)
            .filter(|until| now < *until)
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
            _ if self.snoozed_until(now).is_some() => Severity::Normal,
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
            LabeledValue::new(
                "Warning after active for",
                ValueCell::duration(config.session_warning),
            ),
            LabeledValue::new(
                "Critical after active for",
                ValueCell::duration(config.session_critical),
            ),
            LabeledValue::new("Snooze lasts", ValueCell::duration(config.warning_snooze)),
        ])
        .titled((
            "Session alerts".to_owned(),
            "Alerts follow 'Active since last break'. From the alert, 'Screen off' starts a break \
             at once, 'Lock screen' starts one once input stays idle, and 'Snooze' silences the \
             alert for the time shown."
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
                LabeledValue::new("Active since last break", ValueCell::duration(now - since))
                    .level(self.severity(now))
            }),
            self.accounting.ledger.screen_off_since().map(|since| {
                LabeledValue::new(
                    "Screen off since",
                    ValueCell::Time {
                        at: since,
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                )
            }),
            self.snoozed_until(now).map(|until| {
                LabeledValue::new(
                    "Alert snoozed until",
                    ValueCell::Time {
                        at: until,
                        format: LocalizedTimeFormat::TimeOfDay,
                    },
                )
            }),
            last_break.map(|Interval { until, .. }| {
                LabeledValue::new("Last break ended", ValueCell::RelativeMoment(until))
            }),
            last_break.map(|Interval { from, until }| {
                LabeledValue::new("Last break lasted", ValueCell::duration(until - from))
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

impl GroupItemReading for SessionFace<'_> {
    fn name(&self) -> &'static str {
        "Focus session"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::UserFocus(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, locale: Locale) -> Option<String> {
        self.run_length(clock.utc())
            .map(|run_length| TimeDelta::minutes(run_length.num_minutes()))
            .filter(|whole_minutes| !whole_minutes.is_zero())
            .map(|whole_minutes| duration_text(whole_minutes, locale))
    }

    fn state(&self, clock: Clock) -> ReadingDisplayState {
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

    fn hover_card(&self, clock: Clock) -> HoverCardBody {
        HoverCardBody::sections(self.sections(clock.utc()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sottomano_config::widget_groups::session_focus::SessionFocusConfig;
    use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
        FocusLedger, FocusSignal, Focused,
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

    fn config() -> SessionFocusConfig {
        SessionFocusConfig {
            idle_after: Duration::from_mins(3),
            ..SessionFocusConfig::default()
        }
    }

    #[test]
    fn a_snooze_silences_a_long_session_until_it_ends() {
        let config = SessionFocusConfig {
            session_warning: Duration::from_mins(45),
            ..config()
        };
        let ledger = ledger_after([(focused(1, "kitty"), 0)]);
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            snooze: Some(WarningSnooze { until: at(60) }),
        };
        assert_eq!(face.severity(at(50)), Severity::Normal);
        assert_eq!(face.severity(at(61)), Severity::Warning);
    }

    #[test]
    fn the_summary_names_the_most_used_app_not_the_latest_focus() {
        let config = config();
        let ledger = ledger_after([(focused(1, "firefox"), 0), (focused(2, "kitty"), 40)]);
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            snooze: None,
        };
        let summary = face.screen_run_summary(at(41), Locale::en_US).unwrap();
        assert!(summary.contains("<dt>App</dt><dd>firefox · 40m</dd>"));
        assert!(!summary.contains("kitty"));
    }
}
