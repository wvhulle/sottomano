use chrono::{DateTime, Utc};
use sottomano_sources::login_session_presence::focus_time_ledger::{
    Accounting, FocusUnit, Interval, LAST_DAY,
};

use crate::widgets::tooltip_body::{
    Cell, ColumnHeading, KeyValue, Section, TableRecord, TooltipBody,
};

pub struct WindowUsage<'a> {
    pub accounting: Accounting<'a>,
    pub window: u64,
}

impl WindowUsage<'_> {
    fn this_window(&self, last_day: Interval, now: DateTime<Utc>) -> Section {
        let project = self
            .accounting
            .ledger
            .project_shown_in(self.window)
            .map(|root| KeyValue::new("Project", Cell::Path(root.to_path_buf())));
        let longest = self
            .accounting
            .longest_stay_on(self.window, last_day, now)
            .map(|stay| KeyValue::new("Longest active stay", Cell::duration(stay.length())));
        let entries = [
            project,
            Some(KeyValue::new(
                "Active on this window (24h)",
                Cell::duration(self.accounting.spent_on(self.window, last_day, now)),
            )),
            Some(KeyValue::new(
                "Switched to",
                format!(
                    "{} times",
                    self.accounting.switches_to(self.window, last_day, now)
                ),
            )),
            longest,
        ];
        Section::dictionary(entries.into_iter().flatten()).titled("This window")
    }

    fn projects(&self, last_day: Interval, now: DateTime<Utc>) -> Option<Section> {
        let rows: Vec<TableRecord<1>> = self
            .accounting
            .unit_totals_on(self.window, last_day, now)
            .into_iter()
            .filter(|total| matches!(total.unit, FocusUnit::Project(_)))
            .map(|total| {
                TableRecord::new(
                    total.unit.label().into_owned(),
                    [Some(Cell::duration(total.spent))],
                )
            })
            .collect();
        (rows.len() > 1).then(|| {
            Section::column_table([ColumnHeading::titled("Active (24h)")], rows)
                .titled("Projects in this window")
        })
    }

    pub fn body(&self, now: DateTime<Utc>) -> TooltipBody {
        let last_day = LAST_DAY.ending_at(now);
        TooltipBody::sections(
            [
                Some(self.this_window(last_day, now)),
                self.projects(last_day, now),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
