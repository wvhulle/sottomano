use std::iter::once;

use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{TemperatureInterval, ThermodynamicTemperature},
        temperature_interval, thermodynamic_temperature,
    },
};

use super::core_kind_sections::{PerCore, sections_by_core_kind};
use crate::{
    data::{
        severity::Severity,
        source::sysfs_sensors::temperature::{CoreTemperature, Temperature, ThermalLimit},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        phosphor::Phosphor,
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, untitled},
        widget_state::WidgetState,
    },
};

const WARNING_MARGIN_CELSIUS: f64 = 20.0;
const CRITICAL_MARGIN_CELSIUS: f64 = 5.0;
const UNREPORTED_WARNING_CELSIUS: f64 = 70.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 85.0;

fn celsius_text(temperature: ThermodynamicTemperature) -> String {
    format!(
        "{:.0}",
        temperature.into_format_args(thermodynamic_temperature::degree_celsius, Abbreviation)
    )
}

fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<thermodynamic_temperature::degree_celsius>(degrees)
}

fn margin(degrees: f64) -> TemperatureInterval {
    TemperatureInterval::new::<temperature_interval::degree_celsius>(degrees)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TemperatureThresholds {
    warning: ThermodynamicTemperature,
    critical: ThermodynamicTemperature,
}

impl TemperatureThresholds {
    fn for_limit(limit: ThermalLimit) -> Self {
        match limit {
            ThermalLimit::Reported(throttle) => TemperatureThresholds {
                warning: throttle - margin(WARNING_MARGIN_CELSIUS),
                critical: throttle - margin(CRITICAL_MARGIN_CELSIUS),
            },
            ThermalLimit::Unreported => TemperatureThresholds {
                warning: celsius(UNREPORTED_WARNING_CELSIUS),
                critical: celsius(UNREPORTED_CRITICAL_CELSIUS),
            },
        }
    }

    fn level(self, temperature: ThermodynamicTemperature) -> Severity {
        if temperature > self.critical {
            Severity::Critical
        } else if temperature > self.warning {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

fn reported_limit(limit: ThermalLimit) -> Option<ThermodynamicTemperature> {
    match limit {
        ThermalLimit::Reported(throttle) => Some(throttle),
        ThermalLimit::Unreported => None,
    }
}

impl PerCore for CoreTemperature {
    fn core(&self) -> u32 {
        self.core
    }
}

impl TableRow for CoreTemperature {
    const HEADINGS: &'static [&'static str] = &["Temp", "Limit"];

    fn key(&self) -> Cell {
        format!("Core {}", self.core).into()
    }

    fn cells(&self) -> Vec<Cell> {
        [self.temperature]
            .into_iter()
            .chain(reported_limit(self.limit))
            .map(|temperature| celsius_text(temperature).into())
            .collect()
    }

    fn level(&self) -> Severity {
        TemperatureThresholds::for_limit(self.limit).level(self.temperature)
    }
}

impl Temperature {
    fn thresholds(&self) -> TemperatureThresholds {
        TemperatureThresholds::for_limit(self.throttle_limit)
    }

    fn throttle_row(&self) -> Option<Row> {
        reported_limit(self.throttle_limit)
            .map(|throttle| row("Throttles at", celsius_text(throttle)))
    }
}

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        Phosphor::ThermometerSimple(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        self.thresholds().level(self.hottest()).into()
    }

    fn label(&self) -> Option<String> {
        Some(celsius_text(self.hottest()))
    }

    fn reading_alert_threshold(&self) -> ReadingAlertThreshold {
        ReadingAlertThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "CPU overheating",
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let mut chips: Vec<&str> = self.readings.iter().map(|r| r.chip.as_str()).collect();
        chips.dedup();
        let summary = untitled(
            [
                Some(row("Chips", chips.join(", "))),
                Some(row("Samples", self.readings.len().to_string())),
                self.lowest().map(|lowest| row("Min", celsius_text(lowest))),
                Some(row("Avg", celsius_text(self.average()))),
                Some(row("Max", celsius_text(self.hottest()))),
                self.throttle_row(),
            ]
            .into_iter()
            .flatten()
            .collect(),
        );
        TooltipBody::Sections(
            once(summary)
                .chain(sections_by_core_kind(&self.topology, self.cores.clone()))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reported_limit_sets_thresholds_below_throttling() {
        let thresholds = TemperatureThresholds::for_limit(ThermalLimit::Reported(celsius(100.0)));
        assert_eq!(thresholds.level(celsius(79.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(81.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(96.0)), Severity::Critical);
    }

    #[test]
    fn unreported_limit_falls_back_to_fixed_thresholds() {
        let thresholds = TemperatureThresholds::for_limit(ThermalLimit::Unreported);
        assert_eq!(thresholds.level(celsius(69.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(71.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(86.0)), Severity::Critical);
    }
}
