use chrono::{DateTime, Locale, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::login_session_presence::focus_time_ledger::{
    Accounting, LAST_DAY, LAST_HOUR,
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::{DisplayUnit, Quantity},
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct ContextSwitchRate<'a> {
    pub accounting: Accounting<'a>,
}

impl ContextSwitchRate<'_> {
    fn per_hour(&self, now: DateTime<Utc>) -> usize {
        self.accounting
            .switches_within(LAST_HOUR.ending_at(now), now)
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
        let last_day = LAST_DAY.ending_at(now);
        let switches_last_day = self.accounting.switches_within(last_day, now);
        let active_last_day = self.accounting.screen_spent(last_day, now);
        let average_span = (switches_last_day > 0)
            .then(|| active_last_day / i32::try_from(switches_last_day).unwrap_or(i32::MAX));
        let readings = Section::dictionary(
            [
                Some(
                    KeyValue::new(
                        "Switches in the last hour",
                        Quantity::count(self.per_hour(now)),
                    )
                    .level(self.severity(now)),
                ),
                Some(KeyValue::new(
                    "Switches in the last 24 hours",
                    Quantity::count(switches_last_day),
                )),
                average_span
                    .map(|span| KeyValue::new("Average time per window", Cell::duration(span))),
                Some(KeyValue::new(
                    "Active time in the last hour",
                    Cell::duration(self.accounting.screen_spent(LAST_HOUR.ending_at(now), now)),
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
                Quantity::measured(
                    f64::from(self.accounting.config.switches_per_hour_warning),
                    DisplayUnit::PerHour,
                    0,
                ),
            ),
            KeyValue::new(
                "Critical above per hour",
                Quantity::measured(
                    f64::from(self.accounting.config.switches_per_hour_critical),
                    DisplayUnit::PerHour,
                    0,
                ),
            ),
        ])
        .titled("Switch alerts");
        [readings, thresholds]
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
