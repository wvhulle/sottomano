use crate::widgets::bar_widget::Level;

pub fn level_from_used(used: i64) -> Level {
    if used > 90 {
        Level::Critical
    } else if used > 75 {
        Level::Warning
    } else {
        Level::Ok
    }
}

pub fn level_from_cost(cost: f64) -> Level {
    if cost >= 20.0 {
        Level::Critical
    } else if cost >= 5.0 {
        Level::Warning
    } else {
        Level::Ok
    }
}
