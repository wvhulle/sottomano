use std::cmp::Ordering;

use chrono::{DateTime, TimeDelta, Utc};
use uom::si::f64::Ratio;

use crate::{
    data::{
        severity::Severity,
        source::{
            ai::quota::{Pace, Quota, QuotaLimited},
            focus_ledger::{Accounting, UnitTotal, WakingWindow},
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

    fn runs_out_cell(quota: Option<&Quota>) -> Option<Cell> {
        match quota?.pace()? {
            Pace::RunsOut { at } => Some(Cell::Time {
                at,
                part: TimePart::TimeOfDay,
            }),
            Pace::Lasts { .. } => Some(Cell::from("lasts")),
        }
    }

    fn thresholds(&self) -> Option<Section> {
        self.accounting.screen_budget().map(|budget| {
            Section::dictionary([KeyValue::new("Screen budget", duration_text(budget))])
                .titled("Thresholds")
        })
    }

    pub fn sections(&self) -> Vec<Section> {
        [Some(self.today()), self.thresholds()]
            .into_iter()
            .flatten()
            .collect()
    }

    fn rows(&self, now: DateTime<Utc>) -> Vec<TotalRow> {
        let window = self.waking.around(now);
        let quotas = self.quotas(now);
        let quota_of = |label: &str| quotas.iter().find(|quota| quota.label == label).cloned();
        let units = self.accounting.unit_totals(window, now).into_iter().map(
            |UnitTotal {
                 unit,
                 spent,
                 budget,
             }| TotalRow {
                quota: quota_of(&unit.label()),
                label: unit.label().into_owned(),
                spent,
                budget,
            },
        );
        let screen = TotalRow {
            quota: quota_of("Screen"),
            label: "Screen".to_owned(),
            spent: self.accounting.screen_spent(window, now),
            budget: self.accounting.screen_budget(),
        };
        units.chain([screen]).collect()
    }

    fn today(&self) -> Section {
        let rows = self.rows(Utc::now());
        if rows.iter().any(|row| row.budget.is_some()) {
            Section::column_table(
                [
                    ColumnHeading::new("Today"),
                    ColumnHeading::new("Budget"),
                    ColumnHeading::new("Runs out"),
                ],
                rows.into_iter().map(TotalRow::budgeted_record),
            )
        } else {
            Section::column_table(
                [ColumnHeading::new("Today")],
                rows.into_iter().map(TotalRow::plain_record),
            )
        }
        .titled("Today")
    }
}

struct TotalRow {
    label: String,
    spent: TimeDelta,
    budget: Option<TimeDelta>,
    quota: Option<Quota>,
}

impl TotalRow {
    fn severity(&self) -> Severity {
        self.quota.as_ref().map(Quota::severity).unwrap_or_default()
    }

    fn plain_record(self) -> TableRecord<1> {
        let level = self.severity();
        TableRecord::new(self.label, [Some(duration_text(self.spent).into())]).level(level)
    }

    fn budgeted_record(self) -> TableRecord<3> {
        let level = self.severity();
        TableRecord::new(
            self.label,
            [
                Some(duration_text(self.spent).into()),
                self.budget.map(|budget| duration_text(budget).into()),
                DailyTotalsGauge::runs_out_cell(self.quota.as_ref()),
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
