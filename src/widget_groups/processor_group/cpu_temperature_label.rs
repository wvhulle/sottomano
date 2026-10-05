use std::iter::once;

use uom::si::{
    f64::{TemperatureInterval, ThermodynamicTemperature},
    temperature_interval,
};

use super::core_kind_sections::{PerCore, sections_by_core_kind};
use crate::{
    config::temperature::ThresholdOverrides,
    data::{
        severity::Severity,
        source::sysfs_sensors::temperature::{CoreTemperature, Temperature, ThermalLimit},
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        phosphor::Phosphor,
        temperature_thresholds::{TemperatureThresholds, celsius, celsius_text},
        tooltip_body::{Cell, Row, TableRow, TooltipBody, row, untitled},
        widget_state::WidgetState,
    },
};

const WARNING_MARGIN_CELSIUS: f64 = 20.0;
const CRITICAL_MARGIN_CELSIUS: f64 = 5.0;
const UNREPORTED_WARNING_CELSIUS: f64 = 70.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 85.0;

fn margin(degrees: f64) -> TemperatureInterval {
    TemperatureInterval::new::<temperature_interval::degree_celsius>(degrees)
}

fn cpu_thresholds(limit: ThermalLimit, overrides: ThresholdOverrides) -> TemperatureThresholds {
    let from_hardware = match limit {
        ThermalLimit::Reported(throttle) => TemperatureThresholds {
            warning: throttle - margin(WARNING_MARGIN_CELSIUS),
            critical: throttle - margin(CRITICAL_MARGIN_CELSIUS),
        },
        ThermalLimit::Unreported => TemperatureThresholds {
            warning: celsius(UNREPORTED_WARNING_CELSIUS),
            critical: celsius(UNREPORTED_CRITICAL_CELSIUS),
        },
    };
    from_hardware.overridden_by(overrides)
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
        cpu_thresholds(self.limit, self.overrides).level(self.temperature)
    }
}

impl Temperature {
    fn thresholds(&self) -> TemperatureThresholds {
        cpu_thresholds(self.throttle_limit, self.overrides)
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
                Some(row("Warns above", celsius_text(self.thresholds().warning))),
                Some(row(
                    "Critical above",
                    celsius_text(self.thresholds().critical),
                )),
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
        let thresholds = cpu_thresholds(
            ThermalLimit::Reported(celsius(100.0)),
            ThresholdOverrides::default(),
        );
        assert_eq!(thresholds.level(celsius(79.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(81.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(96.0)), Severity::Critical);
    }

    #[test]
    fn unreported_limit_falls_back_to_fixed_thresholds() {
        let thresholds = cpu_thresholds(ThermalLimit::Unreported, ThresholdOverrides::default());
        assert_eq!(thresholds.level(celsius(69.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(71.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(86.0)), Severity::Critical);
    }
}
