use chrono::Weekday;
use rust_decimal::Decimal;
use sottomano_config::severity::Severity;
use sottomano_sources::ai_usage_spend::{
    spend_money::{Money, usd},
    working_time_forecast::{SpendBudget, SpendPacing},
};

use crate::widgets::{
    localized_quantities::Quantity, tooltip_body::Section, value_cells::LabeledValue,
};

const CLOCK_FORMAT: &str = "%H:%M";

pub const WEEK_HEADING_FORMAT: &str = "Week Since %a %-d %b";

pub const MONTH_HEADING_FORMAT: &str = "%B";

impl Section {
    pub fn spend_forecast_basis(
        sources: impl IntoIterator<Item = LabeledValue>,
        budget: &SpendBudget,
    ) -> Self {
        Section::dictionary(sources.into_iter().chain(pacing_entries(&budget.pacing)))
    }
}

fn pacing_entries(pacing: &SpendPacing) -> Vec<LabeledValue> {
    match pacing {
        SpendPacing::CalendarTime => vec![LabeledValue::new(
            "Projection",
            "cost per hour elapsed so far, every day",
        )],
        SpendPacing::WorkingTime(working_time) => {
            let working_days = working_time
                .days
                .iter(Weekday::Mon)
                .map(|day| day.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let working_hours = format!(
                "{}–{}",
                working_time.hours.start.format(CLOCK_FORMAT),
                working_time.hours.end.format(CLOCK_FORMAT)
            );
            vec![
                LabeledValue::new("Working Days", working_days),
                LabeledValue::new("Working Hours", working_hours),
                LabeledValue::new("Projection", "cost per working hour elapsed so far"),
            ]
        }
    }
}

impl LabeledValue {
    #[must_use]
    pub fn monthly_budget(budget: &SpendBudget, spent: &Money, projected: Option<Money>) -> Self {
        let limit = usd(Decimal::from(budget.monthly_limit_usd));
        let entry = LabeledValue::new("Monthly Budget", Quantity::spend(limit));
        let exceeds_limit = |cost: &Money| cost.amount() > limit.amount();
        if exceeds_limit(spent) || projected.as_ref().is_some_and(exceeds_limit) {
            entry.level(Severity::Warning)
        } else {
            entry
        }
    }
}
