use chrono::{DateTime, Utc};

use crate::{
    data::source::focus_ledger::{Accounting, FocusScope, FocusUnit, Interval, WakingWindow},
    widget_groups::ai_group::amount_text::duration_text,
    widgets::tooltip_body::{Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody},
};

pub struct WindowUsage<'a> {
    pub accounting: Accounting<'a>,
    pub waking: WakingWindow,
    pub window: u64,
}

impl WindowUsage<'_> {
    fn this_window(&self, scope: &FocusScope, today: Interval, now: DateTime<Utc>) -> Section {
        let project = self
            .accounting
            .ledger
            .project_shown_in(self.window)
            .map(|root| KeyValue::new("Project", Cell::Path(root.to_path_buf())));
        let longest = self
            .accounting
            .longest_stay_on(scope, today, now)
            .map(|stay| KeyValue::new("Longest stay", duration_text(stay.length())));
        let entries = [
            project,
            Some(KeyValue::new(
                "Focused today",
                duration_text(self.accounting.spent(scope, today, now)),
            )),
            Some(KeyValue::new(
                "Switched to",
                format!("{} times", self.accounting.switches_to(scope, today, now)),
            )),
            longest,
        ];
        Section::dictionary(entries.into_iter().flatten()).titled("This window")
    }

    fn projects(&self, scope: &FocusScope, today: Interval, now: DateTime<Utc>) -> Option<Section> {
        let rows: Vec<TableRecord<1>> = self
            .accounting
            .unit_totals_within(scope, today, now)
            .into_iter()
            .filter(|total| matches!(total.unit, FocusUnit::Project(_)))
            .map(|total| {
                TableRecord::new(
                    total.unit.label().into_owned(),
                    [Some(duration_text(total.spent).into())],
                )
            })
            .collect();
        (rows.len() > 1).then(|| {
            Section::column_table([ColumnHeading::new("Today")], rows)
                .titled("Projects in this window")
        })
    }

    pub fn body(&self) -> TooltipBody {
        let now = Utc::now();
        let today = self.waking.around(now);
        let scope = FocusScope::Window(self.window);
        TooltipBody::sections(
            [
                Some(self.this_window(&scope, today, now)),
                self.projects(&scope, today, now),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
