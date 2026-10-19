use std::{cmp::Ordering, iter::once};

use chrono::{DateTime, TimeDelta, Utc};
use sottomano_config::severity::Severity;
use sottomano_sources::{
    ai_usage_spend::usage_quota::{Pace, Quota},
    login_session_presence::focus_time_ledger::{
        Accounting, BudgetStatus, FocusUnit, UnitTotal, WakingWindow,
    },
};
use uom::si::f64::Ratio;

use crate::widgets::{
    amount_text::{coarse_duration_text, duration_text},
    bar_widget::{BarWidget, Glyph},
    localized_time::LocalizedTimeFormat,
    phosphor::Phosphor,
    tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
    widget_state::WidgetState,
};

pub struct DailyTotalsGauge {
    screen_spent: TimeDelta,
    screen_budget: Option<BudgetStatus>,
    units: Vec<UnitSpendRecord>,
}

impl DailyTotalsGauge {
    pub fn new(accounting: &Accounting<'_>, waking: WakingWindow) -> Self {
        let now = Utc::now();
        let window = waking.around(now);
        Self {
            screen_spent: accounting.screen_spent(window, now),
            screen_budget: accounting.screen_budget_status(window, now),
            units: accounting
                .unit_totals(window, now)
                .into_iter()
                .map(|total| UnitSpendRecord {
                    budget_status: total.budget_status(window, now),
                    total,
                })
                .collect(),
        }
    }

    fn worst(&self) -> Option<Quota> {
        self.quotas(Utc::now())
            .into_iter()
            .max_by(|a, b| a.used.partial_cmp(&b.used).unwrap_or(Ordering::Equal))
    }

    fn runs_out_cell(quota: &Quota) -> Option<Cell> {
        match quota.pace()? {
            Pace::RunsOut { at } => Some(Cell::Time {
                at,
                format: LocalizedTimeFormat::TimeOfDay,
            }),
            Pace::Lasts { .. } => Some(Cell::from("lasts")),
        }
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
        let today = KeyValue::new("Today", duration_text(self.screen_spent)).level(
            status
                .map(|status| status.quota.severity())
                .unwrap_or_default(),
        );
        let budget_entries = status.into_iter().flat_map(|status| {
            once(KeyValue::new("Budget", duration_text(status.budget))).chain(
                Self::runs_out_cell(&status.quota).map(|cell| KeyValue::new("Runs out", cell)),
            )
        });
        Section::dictionary(once(today).chain(budget_entries)).titled("Screen")
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
                    ColumnHeading::new("Today"),
                    ColumnHeading::new("Budget"),
                    ColumnHeading::new("Runs out"),
                ],
                records.iter().map(|record| Self::budgeted_record(record)),
            ))
        } else {
            Some(Section::column_table(
                [ColumnHeading::new("Today")],
                records.iter().map(|record| Self::plain_record(record)),
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
            [Some(duration_text(self.total.spent).into())],
        )
        .level(level)
    }

    fn budgeted_record(&self) -> TableRecord<3> {
        let level = self.severity();
        TableRecord::new(
            self.total.unit.label().into_owned(),
            [
                Some(duration_text(self.total.spent).into()),
                self.budget_status
                    .as_ref()
                    .map(|status| duration_text(status.budget).into()),
                self.budget_status
                    .as_ref()
                    .and_then(|status| DailyTotalsGauge::runs_out_cell(&status.quota)),
            ],
        )
        .level(level)
    }
}

impl DailyTotalsGauge {
    pub fn quotas(&self, _now: DateTime<Utc>) -> Vec<Quota> {
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
        Phosphor::HourglassMedium(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        let spent = coarse_duration_text(self.screen_spent);
        Some(self.screen_budget.as_ref().map_or(spent.clone(), |status| {
            format!("{spent} / {}", coarse_duration_text(status.budget))
        }))
    }

    fn state(&self) -> WidgetState {
        Quota::worst_severity(&self.quotas(Utc::now()))
            .unwrap_or_default()
            .into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.worst().map(|quota| quota.used)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }

    fn quota_windows(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.quotas(now)
    }
}
