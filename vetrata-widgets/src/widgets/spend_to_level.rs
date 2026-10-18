use rust_decimal::Decimal;
use uom::si::{f64::Ratio, ratio::percent};
use vetrata_config::severity::Severity;
use vetrata_sources::money::Money;

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
