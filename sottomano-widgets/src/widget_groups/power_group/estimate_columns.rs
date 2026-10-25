use chrono::Locale;
use sottomano_sources::battery_power_profiles::battery_time_forecast::Estimate;

use crate::widgets::{
    localized_durations::duration_text,
    tooltip_body::{Cell, ColumnHeading},
};

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

    pub fn label(self, locale: Locale) -> String {
        match self {
            Forecast::Estimated(estimate) => duration_text(estimate.time, locale),
            Forecast::Unavailable(reason) => reason.label().to_string(),
        }
    }

    pub fn cells(self) -> [Option<Cell>; 3] {
        match self {
            Forecast::Estimated(estimate) => [
                Some(Cell::duration(estimate.earliest)),
                Some(Cell::duration(estimate.time)),
                Some(estimate.latest.map_or_else(|| "∞".into(), Cell::duration)),
            ],
            Forecast::Unavailable(reason) => [None, Some(reason.explanation().into()), None],
        }
    }
}
