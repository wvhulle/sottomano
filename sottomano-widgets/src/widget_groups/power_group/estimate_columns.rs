use std::time::Duration;

use sottomano_sources::battery_forecast::Estimate;

use crate::widgets::tooltip_body::{Cell, ColumnHeading};

pub const ESTIMATE_HEADINGS: [ColumnHeading; 3] = [
    ColumnHeading::new("Earliest"),
    ColumnHeading::new("Median"),
    ColumnHeading::new("Latest"),
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Forecast {
    Estimated(Estimate),
    Unavailable(Unforecastable),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Unforecastable {
    LoadingHistory,
    NoChargeHistory,
    ChargerNotKeepingUp,
    NoDrainHistory,
}

impl Unforecastable {
    pub fn label(self) -> &'static str {
        match self {
            Unforecastable::LoadingHistory => "loading",
            Unforecastable::NoChargeHistory | Unforecastable::NoDrainHistory => "learning",
            Unforecastable::ChargerNotKeepingUp => "stalled",
        }
    }

    fn explanation(self) -> &'static str {
        match self {
            Unforecastable::LoadingHistory => "loading learned history",
            Unforecastable::NoChargeHistory => "no charging history learned yet",
            Unforecastable::ChargerNotKeepingUp => "charger barely outpaces the load",
            Unforecastable::NoDrainHistory => "no drain history learned yet",
        }
    }
}

impl Forecast {
    pub fn from_estimate(estimate: Option<Estimate>, otherwise: Unforecastable) -> Self {
        estimate.map_or(Forecast::Unavailable(otherwise), Forecast::Estimated)
    }

    pub fn label(self) -> String {
        match self {
            Forecast::Estimated(estimate) => duration_text(estimate.time),
            Forecast::Unavailable(reason) => reason.label().to_string(),
        }
    }

    pub fn cells(self) -> [Option<Cell>; 3] {
        match self {
            Forecast::Estimated(estimate) => [
                Some(duration_cell(estimate.earliest)),
                Some(duration_cell(estimate.time)),
                Some(estimate.latest.map_or_else(|| "∞".into(), duration_cell)),
            ],
            Forecast::Unavailable(reason) => [None, Some(reason.explanation().into()), None],
        }
    }
}

pub fn duration_text(duration: Duration) -> String {
    humantime::format_duration(duration).to_string()
}

fn duration_cell(duration: Duration) -> Cell {
    duration_text(duration).into()
}
