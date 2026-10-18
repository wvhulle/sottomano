use std::{fs, path::Path};

use crate::data::empty_text::non_empty;

pub mod ccusage;
pub mod claude;
pub mod copilot;
pub mod openrouter;
pub mod quota;
pub mod working_time_forecast;

#[must_use]
pub fn read_secret(path: &Path) -> Option<String> {
    non_empty(fs::read_to_string(path).ok()?.trim().to_owned())
}
