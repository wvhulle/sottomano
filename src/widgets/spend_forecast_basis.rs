use crate::{
    data::source::ai::working_time_forecast::{CalendarSpan, SpendBudget},
    widgets::{amount_text::dollars, tooltip_body::KeyValue},
};

const WEEK_START_FORMAT: &str = "%a %-d %b";
const MONTH_FORMAT: &str = "%B";
const CLOCK_FORMAT: &str = "%H:%M";

impl SpendBudget {
    pub fn forecast_basis(&self) -> impl Iterator<Item = KeyValue> {
        let working_days = self
            .working_days
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let working_hours = format!(
            "{}–{}",
            self.working_hours.start.format(CLOCK_FORMAT),
            self.working_hours.end.format(CLOCK_FORMAT)
        );
        [
            KeyValue::new("Working Days", working_days),
            KeyValue::new("Working Hours", working_hours),
            KeyValue::new("Projection", "cost per working hour elapsed so far"),
        ]
        .into_iter()
        .chain(
            self.monthly_limit_usd
                .map(|limit| KeyValue::new("Monthly Budget", dollars(f64::from(limit)))),
        )
    }
}

impl CalendarSpan {
    pub fn week_heading(self) -> String {
        format!("Week Since {}", self.since.format(WEEK_START_FORMAT))
    }

    pub fn month_heading(self) -> String {
        self.since.format(MONTH_FORMAT).to_string()
    }
}
