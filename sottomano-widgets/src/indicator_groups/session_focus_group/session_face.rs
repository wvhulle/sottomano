use chrono::{DateTime, Locale, TimeDelta, Utc};
use quick_xml::escape::escape;
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, FocusUnit, Interval, ScreenRun, UnitTotal,
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
    },
    hover_cards::hover_card_body::{Section, SectionTitle, dictionary_table},
    indicator_groups::session_focus_group::WarningSnooze,
    widgets::{
        localized_durations::duration_text,
        localized_time::LocalizedTimeFormat,
        ongoing_toast::{
            IndicatorAlerts, NotificationBody, NotificationProgress, StateToast, definition_list,
            moment_text,
        },
        value_cells::{LabeledValue, ValueCell},
    },
};

pub struct SessionFace<'view> {
    pub accounting: Accounting<'view>,
    pub snooze: WarningSnooze,
}

impl SessionFace<'_> {
    pub(super) fn snoozed_until(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self.snooze {
            WarningSnooze::SnoozedUntil(until) if now < until => Some(until),
            WarningSnooze::SnoozedUntil(_) | WarningSnooze::NotSnoozed => None,
        }
    }

    fn run_length(&self, now: DateTime<Utc>) -> Option<TimeDelta> {
        self.accounting
            .screen_run(now)
            .map(|ScreenRun { since }| now - since)
    }

    pub(super) fn severity(&self, now: DateTime<Utc>) -> Severity {
        let config = self.accounting.config;
        let critical = config.session_critical;
        let warning = config.session_warning;
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
        let unit_rows = |name_key, spent_key, total: &UnitTotal| {
            [
                (name_key, escape(total.unit.label().as_ref()).into_owned()),
                (spent_key, duration_text(total.spent, locale)),
            ]
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
            .chain(
                most_used_project
                    .into_iter()
                    .flat_map(|total| unit_rows("Project", "Time on project", total)),
            )
            .chain(
                most_used_app
                    .into_iter()
                    .flat_map(|total| unit_rows("App", "Time in app", total)),
            ),
        ))
    }

    fn session_alerts(&self) -> Section {
        let config = self.accounting.config;
        Section::new(
            SectionTitle::captioned(
                "Session alerts".to_owned(),
                "Alerts follow 'Active since last break'. From the alert, 'Screen off' starts a \
                 break at once, 'Lock screen' starts one once input stays idle, and 'Snooze' \
                 silences the alert for the time shown."
                    .to_owned(),
            ),
            dictionary_table([
                LabeledValue::new(
                    "Warning after active for",
                    ValueCell::duration(config.session_warning),
                ),
                LabeledValue::new(
                    "Critical after active for",
                    ValueCell::duration(config.session_critical),
                ),
                LabeledValue::new("Snooze lasts", ValueCell::duration(config.warning_snooze)),
            ]),
        )
    }

    pub fn sections(&self, now: DateTime<Utc>) -> [Section; 2] {
        [self.current_session(now), self.session_alerts()]
    }

    pub(super) fn long_session_alerts(
        &self,
        now: DateTime<Utc>,
        locale: Locale,
    ) -> IndicatorAlerts {
        let body = match self.screen_run_summary(now, locale) {
            Some(summary) => NotificationBody::Text(summary),
            None => NotificationBody::Bodiless,
        };
        IndicatorAlerts {
            state_toast: StateToast::NotifyFromSeverity {
                minimum_severity: Severity::Warning,
                summary_text: "Long focus session",
                body,
                progress: NotificationProgress::Unmeasured,
            },
            ..IndicatorAlerts::default()
        }
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
        Section::new(
            SectionTitle::captioned(
                "Current session".to_owned(),
                "Active means recent keyboard or pointer input. The count starts when the last \
                 break ended; the break rows show what counts as one."
                    .to_owned(),
            ),
            dictionary_table(entries.into_iter().flatten()),
        )
    }
}

impl<'view> SessionFace<'view> {
    pub fn indicator(self, clock: Clock, locale: Locale) -> GroupIndicator<'view> {
        let label = self
            .run_length(clock.utc())
            .map(|run_length| TimeDelta::minutes(run_length.num_minutes()))
            .filter(|whole_minutes| !whole_minutes.is_zero())
            .map(|whole_minutes| duration_text(whole_minutes, locale));
        let alerts = self.long_session_alerts(clock.utc(), locale);
        GroupIndicator::glyph_only(
            "Current focus",
            Phosphor::UserFocus(Weight::Regular).into(),
            self.severity(clock.utc()).into(),
        )
        .with_readout(IndicatorReadout {
            label: match label {
                Some(text) if !text.is_empty() => ReadoutLabel::Labeled(text),
                Some(_) | None => ReadoutLabel::Unlabeled,
            },
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_alerts(alerts)
        .with_hover_card(Box::new(move |card_clock| self.hover_card(card_clock)))
    }

    pub fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.sections(clock.utc()).into_iter().collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
        FocusLedger, FocusProject, FocusSignal, Focused,
    };
    use sottomano_toml_config::indicator_groups::session_focus::SessionFocusConfig;

    use super::*;

    fn at(minutes: i64) -> DateTime<Utc> {
        "2026-10-14T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
    }

    fn focused(window: u64, app_id: &str) -> FocusSignal {
        FocusSignal::WindowFocused(Focused {
            window,
            app_id: app_id.to_owned(),
            project: FocusProject::OutsideRepository,
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
            idle_after: TimeDelta::minutes(3),
            ..SessionFocusConfig::default()
        }
    }

    #[test]
    fn a_snooze_silences_a_long_session_until_it_ends() {
        let config = SessionFocusConfig {
            session_warning: TimeDelta::minutes(45),
            ..config()
        };
        let ledger = ledger_after([(focused(1, "kitty"), 0)]);
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            snooze: WarningSnooze::SnoozedUntil(at(60)),
        };
        assert_eq!(face.severity(at(50)), Severity::Normal);
        assert_eq!(face.severity(at(61)), Severity::Warning);
    }

    #[test]
    fn the_headline_names_the_most_used_app_not_the_latest_focus() {
        let config = config();
        let ledger = ledger_after([(focused(1, "firefox"), 0), (focused(2, "kitty"), 40)]);
        let face = SessionFace {
            accounting: ledger.accounted(&config),
            snooze: WarningSnooze::NotSnoozed,
        };
        let summary = face.screen_run_summary(at(41), Locale::en_US).unwrap();
        assert!(summary.contains("<dt>App</dt><dd>firefox</dd>"));
        assert!(summary.contains("<dt>Time in app</dt><dd>40m</dd>"));
        assert!(!summary.contains("kitty"));
    }
}
