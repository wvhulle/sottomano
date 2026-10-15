use std::{cmp::Ordering, iter::once};

use chrono::{DateTime, Utc};
use uom::si::f64::Ratio;

use crate::{
    data::{
        severity::Severity,
        source::{
            ai::quota::{Pace, Quota, QuotaLimited},
            focus_ledger::{Accounting, FocusUnit, Interval, UnitTotal, WakingWindow},
        },
    },
    widget_groups::ai_group::amount_text::{coarse_duration_text, duration_text},
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct DailyTotalsGauge<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
}

impl DailyTotalsGauge<'_> {
    fn worst(&self, now: DateTime<Utc>) -> Option<Quota> {
        self.quotas(now)
            .into_iter()
            .max_by(|a, b| a.used.partial_cmp(&b.used).unwrap_or(Ordering::Equal))
    }

    fn runs_out_cell(quota: &Quota) -> Option<Cell> {
        match quota.pace()? {
            Pace::RunsOut { at } => Some(Cell::Time {
                at,
                part: TimePart::TimeOfDay,
            }),
            Pace::Lasts { .. } => Some(Cell::from("lasts")),
        }
    }

    pub fn sections(&self) -> Vec<Section> {
        let now = Utc::now();
        let window = self.waking.around(now);
        let (apps, projects): (Vec<UnitSpendRecord>, Vec<UnitSpendRecord>) = self
            .accounting
            .unit_totals(window, now)
            .into_iter()
            .map(|total| UnitSpendRecord {
                quota: total.quota(window, now),
                total,
            })
            .partition(|record| matches!(record.total.unit, FocusUnit::App(_)));
        once(self.screen(window, now))
            .chain(UnitSpendRecord::section(apps, "Applications"))
            .chain(UnitSpendRecord::section(projects, "Projects"))
            .collect()
    }

    fn screen(&self, window: Interval, now: DateTime<Utc>) -> Section {
        let quota = self.accounting.screen_quota(window, now);
        let today = KeyValue::new(
            "Today",
            duration_text(self.accounting.screen_spent(window, now)),
        )
        .level(quota.as_ref().map(Quota::severity).unwrap_or_default());
        let budget = self
            .accounting
            .screen_budget()
            .map(|budget| KeyValue::new("Budget", duration_text(budget)));
        let runs_out = quota
            .as_ref()
            .and_then(Self::runs_out_cell)
            .map(|cell| KeyValue::new("Runs out", cell));
        Section::dictionary(once(today).chain(budget).chain(runs_out)).titled("Screen")
    }
}

struct UnitSpendRecord {
    total: UnitTotal,
    quota: Option<Quota>,
}

impl UnitSpendRecord {
    fn section(records: Vec<Self>, title: &str) -> Option<Section> {
        if records.is_empty() {
            None
        } else if records.iter().any(|record| record.total.budget.is_some()) {
            Some(Section::column_table(
                [
                    ColumnHeading::new("Today"),
                    ColumnHeading::new("Budget"),
                    ColumnHeading::new("Runs out"),
                ],
                records.into_iter().map(Self::budgeted_record),
            ))
        } else {
            Some(Section::column_table(
                [ColumnHeading::new("Today")],
                records.into_iter().map(Self::plain_record),
            ))
        }
        .map(|section| section.titled(title))
    }

    fn severity(&self) -> Severity {
        self.quota.as_ref().map(Quota::severity).unwrap_or_default()
    }

    fn plain_record(self) -> TableRecord<1> {
        let level = self.severity();
        TableRecord::new(
            self.total.unit.label().into_owned(),
            [Some(duration_text(self.total.spent).into())],
        )
        .level(level)
    }

    fn budgeted_record(self) -> TableRecord<3> {
        let level = self.severity();
        TableRecord::new(
            self.total.unit.label().into_owned(),
            [
                Some(duration_text(self.total.spent).into()),
                self.total.budget.map(|budget| duration_text(budget).into()),
                self.quota
                    .as_ref()
                    .and_then(DailyTotalsGauge::runs_out_cell),
            ],
        )
        .level(level)
    }
}

impl QuotaLimited for DailyTotalsGauge<'_> {
    fn quotas(&self, now: DateTime<Utc>) -> Vec<Quota> {
        self.accounting.quotas_within(self.waking.around(now), now)
    }
}

impl BarWidget for DailyTotalsGauge<'_> {
    fn name(&self) -> &'static str {
        "Daily focus budget"
    }

    fn icon(&self) -> Glyph {
        Phosphor::HourglassMedium(self.state().weight()).into()
    }

    fn label(&self) -> Option<String> {
        let now = Utc::now();
        let spent =
            coarse_duration_text(self.accounting.screen_spent(self.waking.around(now), now));
        Some(
            self.accounting
                .screen_budget()
                .map_or(spent.clone(), |budget| {
                    format!("{spent} / {}", coarse_duration_text(budget))
                }),
        )
    }

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now()).unwrap_or_default().into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.worst(Utc::now()).map(|quota| quota.used)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(self.sections())
    }
}
