use crate::widgets::widget_state::Level;

pub fn level_from_used(used_percent: f64) -> Level {
    if used_percent > 90.0 {
        Level::Critical
    } else if used_percent > 75.0 {
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
