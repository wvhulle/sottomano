use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::data::source::{BarWidget, Level};

pub mod claude;
pub mod copilot;
pub mod http;
pub mod openrouter;

pub trait ApiSource: BarWidget + Clone + PartialEq + Sized + Send + 'static {
    const NAME: &'static str;
    const INTERVAL: Duration;

    fn poll(key: Option<PathBuf>) -> Option<Self>;
}

pub fn read_secret(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub fn level_from_remaining(remaining: i64) -> Level {
    if remaining < 10 {
        Level::Critical
    } else if remaining < 25 {
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
