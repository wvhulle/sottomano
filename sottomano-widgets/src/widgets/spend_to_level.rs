use rust_decimal::Decimal;
use sottomano_config::severity::Severity;
use sottomano_sources::ai_usage_spend::spend_money::Money;
use uom::si::{f64::Ratio, ratio::percent};

#[must_use]
pub fn level_from_used(used: Ratio) -> Severity {
    if used > Ratio::new::<percent>(90.0) {
        Severity::Critical
    } else if used > Ratio::new::<percent>(75.0) {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

#[must_use]
pub fn level_from_cost(cost: &Money) -> Severity {
    if *cost.amount() >= Decimal::from(20) {
        Severity::Critical
    } else if *cost.amount() >= Decimal::from(5) {
        Severity::Warning
    } else {
        Severity::Normal
    }
}
