use std::{cmp::Ordering, iter::once};

use chrono::{DateTime, Locale, TimeDelta, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::{
    ai_usage_spend::usage_quota::Quota,
    login_session_presence::focus_time_ledger::{
        Accounting, BudgetStatus, FocusUnit, LAST_DAY, UnitTotal,
    },
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_durations::duration_text,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct DailyTotalsGauge {
    computed_at: DateTime<Utc>,
    screen_spent: TimeDelta,
    screen_budget: Option<BudgetStatus>,
    units: Vec<UnitSpendRecord>,
}

impl DailyTotalsGauge {
    pub fn new(accounting: &Accounting<'_>, now: DateTime<Utc>) -> Self {
        let last_day = LAST_DAY.ending_at(now);
        Self {
            computed_at: now,
            screen_spent: accounting.screen_spent(last_day, now),
            screen_budget: accounting.screen_budget_status(now),
            units: accounting
                .unit_totals(last_day, now)
                .into_iter()
                .map(|total| UnitSpendRecord {
                    budget_status: total.budget_status(now),
                    total,
                })
                .collect(),
        }
    }

    pub fn computed_in_second_of(&self, now: DateTime<Utc>) -> bool {
        self.computed_at.timestamp() == now.timestamp()
    }

    fn worst(&self) -> Option<Quota> {
        self.quotas()
            .into_iter()
            .max_by(|a, b| a.used.partial_cmp(&b.used).unwrap_or(Ordering::Equal))
    }

    pub fn sections(&self) -> Vec<Section> {
        let (apps, projects): (Vec<&UnitSpendRecord>, Vec<&UnitSpendRecord>) = self
            .units
            .iter()
            .partition(|record| matches!(record.total.unit, FocusUnit::App(_)));
        once(self.screen())
            .chain(UnitSpendRecord::section(&apps, "Applications"))
            .chain(UnitSpendRecord::section(&projects, "Projects"))
            .collect()
    }

    fn screen(&self) -> Section {
        let status = self.screen_budget.as_ref();
        let active = KeyValue::new("Active (24h)", Cell::duration(self.screen_spent)).level(
            status
                .map(|status| status.quota.severity())
                .unwrap_or_default(),
        );
        let budget_entry =
            status.map(|status| KeyValue::new("Daily budget", Cell::duration(status.budget)));
        Section::dictionary(once(active).chain(budget_entry)).titled((
            "Screen".to_owned(),
            "Only active time in the last 24 hours counts, attributed to the focused window's app \
             and project. A budget turns critical once that time exceeds it."
                .to_owned(),
        ))
    }
}

struct UnitSpendRecord {
    total: UnitTotal,
    budget_status: Option<BudgetStatus>,
}

impl UnitSpendRecord {
    fn section(records: &[&Self], title: &str) -> Option<Section> {
        if records.is_empty() {
            None
        } else if records.iter().any(|record| record.budget_status.is_some()) {
            Some(Section::column_table(
                [
                    ColumnHeading::titled("Active (24h)"),
                    ColumnHeading::titled("Daily budget"),
                ],
                records.iter().map(|record| record.budgeted_record()),
            ))
        } else {
            Some(Section::column_table(
                [ColumnHeading::titled("Active (24h)")],
                records.iter().map(|record| record.plain_record()),
            ))
        }
        .map(|section| section.titled(title))
    }

    fn severity(&self) -> Severity {
        self.budget_status
            .as_ref()
            .map(|status| status.quota.severity())
            .unwrap_or_default()
    }

    fn plain_record(&self) -> TableRecord<1> {
        let level = self.severity();
        TableRecord::new(
            self.total.unit.label().into_owned(),
            [Some(Cell::duration(self.total.spent))],
        )
        .level(level)
    }

    fn budgeted_record(&self) -> TableRecord<2> {
        let level = self.severity();
        TableRecord::new(
            self.total.unit.label().into_owned(),
            [
                Some(Cell::duration(self.total.spent)),
                self.budget_status
                    .as_ref()
                    .map(|status| Cell::duration(status.budget)),
            ],
        )
        .level(level)
    }
}

impl DailyTotalsGauge {
    pub fn quotas(&self) -> Vec<Quota> {
        self.units
            .iter()
            .filter_map(|record| record.budget_status.as_ref())
            .chain(&self.screen_budget)
            .map(|status| status.quota.clone())
            .collect()
    }
}

impl BarWidget for DailyTotalsGauge {
    fn name(&self) -> &'static str {
        "Daily focus budget"
    }

    fn icon(&self) -> Glyph {
        Phosphor::HourglassMedium(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        let coarse = |length| duration_text(length, locale);
        let spent = coarse(self.screen_spent);
        Some(match &self.screen_budget {
            Some(status) => format!("{spent} / {}", coarse(status.budget)),
            None => spent,
        })
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        Quota::worst_severity(&self.quotas())
            .unwrap_or_default()
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.worst().map(|quota| quota.used)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }

    fn quota_windows(&self, _now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas()
    }
}
