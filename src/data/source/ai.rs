use std::{fs, path::Path};

use crate::data::source::Level;

pub mod claude;
pub mod copilot;
pub mod openrouter;

pub fn read_secret(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

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
