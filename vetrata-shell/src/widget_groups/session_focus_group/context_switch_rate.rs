use chrono::{DateTime, TimeDelta, Utc};

use crate::{
    data::{
        severity::Severity,
        source::focus_ledger::{Accounting, Interval, WakingWindow},
    },
    widgets::{
        amount_text::duration_text,
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct ContextSwitchRate<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
}

impl ContextSwitchRate<'_> {
    fn last_hour(now: DateTime<Utc>) -> Interval {
        Interval {
            from: now - TimeDelta::hours(1),
            until: now,
        }
    }

    fn per_hour(&self, now: DateTime<Utc>) -> usize {
        self.accounting.switches_within(Self::last_hour(now), now)
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

    pub fn sections(&self) -> [Section; 2] {
        let now = Utc::now();
        let last_hour = Self::last_hour(now);
        let today = self.waking.around(now);
        let switches_today = self.accounting.switches_within(today, now);
        let active_today = self.accounting.screen_spent(today, now);
        let average_span = (switches_today > 0)
            .then(|| active_today / i32::try_from(switches_today).unwrap_or(i32::MAX));
        let readings = Section::dictionary(
            [
                Some(
                    KeyValue::new("Last hour", self.per_hour(now).to_string())
                        .level(self.severity(now)),
                ),
                Some(KeyValue::new("Today", switches_today.to_string())),
                average_span.map(|span| KeyValue::new("Average stay", duration_text(span))),
                Some(KeyValue::new(
                    "Active in the last hour",
                    duration_text(self.accounting.screen_spent(last_hour, now)),
                )),
            ]
            .into_iter()
            .flatten(),
        )
        .titled("Switches");
        let thresholds = Section::dictionary([
            KeyValue::new(
                "Warns above",
                format!("{}/h", self.accounting.config.switches_per_hour_warning),
            ),
            KeyValue::new(
                "Critical above",
                format!("{}/h", self.accounting.config.switches_per_hour_critical),
            ),
        ])
        .titled("Thresholds");
        [readings, thresholds]
    }
}

impl BarWidget for ContextSwitchRate<'_> {
    fn name(&self) -> &'static str {
        "Context switches"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Shuffle(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        Some(format!("{}/h", self.per_hour(Utc::now())))
    }

    fn state(&self) -> WidgetState {
        self.severity(Utc::now()).into()
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }
}
