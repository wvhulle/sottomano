use chrono::{DateTime, Locale, TimeDelta, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::login_session_presence::focus_time_ledger::{
    Accounting, Interval, WakingWindow,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct ContextSwitchRate<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
}

impl ContextSwitchRate<'_> {
    fn per_hour(&self, now: DateTime<Utc>) -> usize {
        self.accounting.switches_within(last_hour(now), now)
    }

    fn severity(&self, now: DateTime<Utc>) -> Severity {
        let config = self.accounting.config;
        let critical = usize::try_from(config.switches_per_hour_critical).unwrap_or(usize::MAX);
        let warning = usize::try_from(config.switches_per_hour_warning).unwrap_or(usize::MAX);
        match self.per_hour(now) {
            rate if rate > critical => Severity::Critical,
            rate if rate > warning => Severity::Warning,
            _ => Severity::Normal,
        }
    }

    pub fn sections(&self, now: DateTime<Utc>) -> [Section; 2] {
        let last_hour = last_hour(now);
        let today = self.waking.around(now);
        let switches_today = self.accounting.switches_within(today, now);
        let active_today = self.accounting.screen_spent(today, now);
        let average_span = (switches_today > 0)
            .then(|| active_today / i32::try_from(switches_today).unwrap_or(i32::MAX));
        let readings = Section::dictionary(
            [
                Some(
                    KeyValue::new("Switches in the last hour", self.per_hour(now).to_string())
                        .level(self.severity(now)),
                ),
                Some(KeyValue::new("Switches today", switches_today.to_string())),
                average_span
                    .map(|span| KeyValue::new("Average time per window", Cell::duration(span))),
                Some(KeyValue::new(
                    "Active time in the last hour",
                    Cell::duration(self.accounting.screen_spent(last_hour, now)),
                )),
            ]
            .into_iter()
            .flatten(),
        )
        .titled((
            "Window switches".to_owned(),
            "A switch is focusing another window, or the same window in another app or project; \
             refocusing the same one doesn't count."
                .to_owned(),
        ));
        let thresholds = Section::dictionary([
            KeyValue::new(
                "Warning above per hour",
                format!("{}/h", self.accounting.config.switches_per_hour_warning),
            ),
            KeyValue::new(
                "Critical above per hour",
                format!("{}/h", self.accounting.config.switches_per_hour_critical),
            ),
        ])
        .titled("Switch alerts");
        [readings, thresholds]
    }
}

fn last_hour(now: DateTime<Utc>) -> Interval {
    Interval {
        from: now - TimeDelta::hours(1),
        until: now,
    }
}

impl BarWidget for ContextSwitchRate<'_> {
    fn name(&self) -> &'static str {
        "Context switches"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Shuffle(Weight::Regular).into()
    }

    fn label(&self, clock: Clock, _locale: Locale) -> Option<String> {
        Some(format!("{}/h", self.per_hour(clock.utc())))
    }

    fn state(&self, clock: Clock) -> WidgetState {
        self.severity(clock.utc()).into()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        TooltipBody::sections(self.sections(clock.utc()))
    }
}
