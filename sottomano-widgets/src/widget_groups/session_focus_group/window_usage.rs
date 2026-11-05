use chrono::{DateTime, Utc};
use sottomano_desktop_sources::login_session_presence::focus_time_ledger::{
    Accounting, FocusUnit, Interval, LAST_DAY,
};

use crate::{
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        keyed_record_table::{ColumnHeading, TableRecord},
        value_cells::{LabeledValue, ValueCell},
    },
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
            .map(|root| LabeledValue::new("Project", ValueCell::Path(root.to_path_buf())));
        let longest = self
            .accounting
            .longest_stay_on(self.window, last_day, now)
            .map(|stay| {
                LabeledValue::new("Longest active stay", ValueCell::duration(stay.length()))
            });
        let entries = [
            project,
            Some(LabeledValue::new(
                "Active on this window (24h)",
                ValueCell::duration(self.accounting.spent_on(self.window, last_day, now)),
            )),
            Some(LabeledValue::new(
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
                    [ValueCell::duration(total.spent)],
                )
            })
            .collect();
        (rows.len() > 1).then(|| {
            Section::column_table([ColumnHeading::titled("Active (24h)")], rows)
                .titled("Projects in this window")
        })
    }

    pub fn body(&self, now: DateTime<Utc>) -> HoverCardBody {
        let last_day = LAST_DAY.ending_at(now);
        HoverCardBody::sections(
            [
                Some(self.this_window(last_day, now)),
                self.projects(last_day, now),
            ]
            .into_iter()
            .flatten(),
        )
    }
}
