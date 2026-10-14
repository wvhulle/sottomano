use chrono::{DateTime, TimeDelta, Utc};
use uom::si::{f64::Ratio, ratio::ratio};

use crate::{
    data::source::{
        ai::quota::{Pace, Quota},
        focus_ledger::{Accounting, FocusUnit, UnitTotal, WakingWindow},
    },
    widget_groups::ai_group::amount_text::{duration_text, percent_text},
    widgets::{
        localized_time::TimePart,
        tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
    },
};

pub struct AppUsage<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
    pub app_id: String,
}

impl AppUsage<'_> {
    fn runs_out(quota: &Quota) -> Option<KeyValue> {
        match quota.pace()? {
            Pace::RunsOut { at } => Some(KeyValue::new(
                "Runs out",
                Cell::Time {
                    at,
                    part: TimePart::TimeOfDay,
                },
            )),
            Pace::Lasts { .. } => Some(KeyValue::new("Runs out", "lasts")),
        }
    }

    fn usage(&self, unit: &FocusUnit, now: DateTime<Utc>) -> Section {
        let window = self.waking.around(now);
        let spent = self.accounting.spent(unit, window, now);
        let screen = self.accounting.screen_spent(window, now);
        let share = (screen > TimeDelta::zero())
            .then(|| Ratio::new::<ratio>(spent.as_seconds_f64() / screen.as_seconds_f64()));
        let quota = self
            .accounting
            .quotas_within(window, now)
            .into_iter()
            .find(|quota| quota.label == self.app_id);
        let entries = [
            Some(KeyValue::new("Today", duration_text(spent))),
            share.map(|share| KeyValue::new("Share of screen time", percent_text(share))),
            quota.as_ref().and_then(|quota| {
                unit.budget(self.accounting.config)
                    .map(|budget| KeyValue::new("Budget", duration_text(budget)))
                    .map(|entry| entry.level(quota.severity()))
            }),
            quota.as_ref().and_then(Self::runs_out),
            Some(KeyValue::new(
                "Switched to",
                format!("{} times", self.accounting.switches_to(unit, window, now)),
            )),
            self.accounting
                .longest_stay_on(unit, window, now)
                .map(|stay| KeyValue::new("Longest stay", duration_text(stay.length()))),
        ];
        Section::dictionary(entries.into_iter().flatten()).titled("Usage today")
    }

    fn projects(&self, unit: &FocusUnit, now: DateTime<Utc>) -> Option<Section> {
        let window = self.waking.around(now);
        let rows: Vec<TableRecord<2>> = self
            .accounting
            .unit_totals_with(unit, window, now)
            .into_iter()
            .filter(|total| matches!(total.unit, FocusUnit::Project(_)))
            .map(
                |UnitTotal {
                     unit,
                     spent,
                     budget,
                 }| {
                    TableRecord::new(
                        unit.label().into_owned(),
                        [
                            Some(duration_text(spent).into()),
                            budget.map(|budget| duration_text(budget).into()),
                        ],
                    )
                },
            )
            .collect();
        (!rows.is_empty()).then(|| {
            Section::column_table(
                [ColumnHeading::new("Today"), ColumnHeading::new("Budget")],
                rows,
            )
            .titled("Projects")
        })
    }

    pub fn body(&self) -> TooltipBody {
        let now = Utc::now();
        let unit = FocusUnit::App(self.app_id.clone());
        TooltipBody::sections(
            [Some(self.usage(&unit, now)), self.projects(&unit, now)]
                .into_iter()
                .flatten(),
        )
    }
}
