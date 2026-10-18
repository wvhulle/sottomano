use rust_decimal::Decimal;
use sottomano_config::severity::Severity;
use sottomano_sources::{
    ai::working_time_forecast::SpendBudget,
    money::{Money, usd},
};

use crate::widgets::tooltip_body::{KeyValue, Section};

const CLOCK_FORMAT: &str = "%H:%M";

pub const WEEK_HEADING_FORMAT: &str = "Week Since %a %-d %b";

pub const MONTH_HEADING_FORMAT: &str = "%B";

impl Section {
    pub fn spend_forecast_basis(
        sources: impl IntoIterator<Item = KeyValue>,
        budget: &SpendBudget,
    ) -> Self {
        let working_days = budget
            .working_days
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let working_hours = format!(
            "{}–{}",
            budget.working_hours.start.format(CLOCK_FORMAT),
            budget.working_hours.end.format(CLOCK_FORMAT)
        );
        Section::dictionary(sources.into_iter().chain([
            KeyValue::new("Working Days", working_days),
            KeyValue::new("Working Hours", working_hours),
            KeyValue::new("Projection", "cost per working hour elapsed so far"),
        ]))
    }
}

impl KeyValue {
    #[must_use]
    pub fn monthly_budget(budget: &SpendBudget, spent: &Money, projected: Option<Money>) -> Self {
        let limit = usd(Decimal::from(budget.monthly_limit_usd));
        let entry = KeyValue::new("Monthly Budget", limit.to_string());
        let exceeds_limit = |cost: &Money| cost.amount() > limit.amount();
        if exceeds_limit(spent) || projected.as_ref().is_some_and(exceeds_limit) {
            entry.level(Severity::Warning)
        } else {
            entry
        }
    }
}
