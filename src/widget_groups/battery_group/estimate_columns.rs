use std::time::Duration;

use crate::{data::battery_forecast::Estimate, widgets::tooltip_body::Cell};

pub const ESTIMATE_HEADINGS: &[&str] = &["Median", "Earliest", "Latest"];

pub fn duration_text(duration: Duration) -> String {
    humantime::format_duration(duration).to_string()
}

fn duration_cell(duration: Duration) -> Cell {
    duration_text(duration).into()
}

pub fn estimate_cells(estimate: Option<Estimate>) -> Vec<Cell> {
    let Some(estimate) = estimate else {
        return vec!["—".into()];
    };
    vec![
        duration_cell(estimate.time),
        duration_cell(estimate.earliest),
        estimate.latest.map_or_else(|| "∞".into(), duration_cell),
    ]
}
