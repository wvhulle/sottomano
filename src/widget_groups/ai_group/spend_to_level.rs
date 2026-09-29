use crate::data::severity::Severity;

pub fn level_from_used(used_percent: f64) -> Severity {
    if used_percent > 90.0 {
        Severity::Critical
    } else if used_percent > 75.0 {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

pub fn level_from_cost(cost: f64) -> Severity {
    if cost >= 20.0 {
        Severity::Critical
    } else if cost >= 5.0 {
        Severity::Warning
    } else {
        Severity::Normal
    }
}
