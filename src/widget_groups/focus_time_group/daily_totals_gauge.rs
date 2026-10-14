use std::cmp::Ordering;

use chrono::{DateTime, TimeDelta, Utc};
use uom::si::f64::Ratio;

use crate::{
    data::source::{
        ai::quota::{Pace, Quota, QuotaLimited},
        focus_ledger::{Accounting, UnitTotal, WakingWindow},
    },
    widget_groups::ai_group::amount_text::duration_text,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_time::TimePart,
        phosphor::Phosphor,
        tooltip_body::{Cell, ColumnHeading, Section, TableRecord, TooltipBody},
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

    fn record(
        quotas: &[Quota],
        label: &str,
        spent: TimeDelta,
        budget: Option<TimeDelta>,
    ) -> TableRecord<3> {
        let quota = quotas.iter().find(|quota| quota.label == label);
        TableRecord::new(
            label.to_owned(),
            [
                Some(duration_text(spent).into()),
                budget.map(|budget| duration_text(budget).into()),
                Self::runs_out_cell(quota),
            ],
        )
        .level(quota.map(Quota::severity).unwrap_or_default())
    }

    pub fn section(&self) -> Section {
        let now = Utc::now();
        let window = self.waking.around(now);
        let quotas = self.quotas(now);
        let units = self.accounting.unit_totals(window, now).into_iter().map(
            |UnitTotal {
                 unit,
                 spent,
                 budget,
             }| { Self::record(&quotas, &unit.label(), spent, budget) },
        );
        let screen = Self::record(
            &quotas,
            "Screen",
            self.accounting.screen_spent(window, now),
            self.accounting.screen_budget(),
        );
        Section::column_table(
            [
                ColumnHeading::new("Today"),
                ColumnHeading::new("Budget"),
                ColumnHeading::new("Runs out"),
            ],
            units.chain([screen]),
        )
        .titled((
            "Today",
            format!(
                "{} to {}",
                self.waking.start.format("%H:%M"),
                self.waking.end.format("%H:%M")
            ),
        ))
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

    fn state(&self) -> WidgetState {
        self.quota_severity(Utc::now()).unwrap_or_default().into()
    }

    fn gauge(&self) -> Option<Ratio> {
        self.worst(Utc::now()).map(|quota| quota.used)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections([self.section()])
    }
}
