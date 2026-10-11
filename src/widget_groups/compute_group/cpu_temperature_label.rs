use uom::si::{
    f64::{TemperatureInterval, ThermodynamicTemperature},
    temperature_interval,
};

use super::core_kind_sections::{PerCore, sections_by_core_kind};
use crate::{
    config::widget_groups::temperature_sensors::ThresholdOverrides,
    data::{
        severity::Severity,
        source::sysfs_sensors::{
            cpu_topology::CpuTopology,
            temperature::{CoreTemperature, CpuCoreTemperatures, Temperature},
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        phosphor::Phosphor,
        temperature_thresholds::{TemperatureThresholds, celsius, celsius_text},
        tooltip_body::{KeyValue, Section, TableRecord, TooltipBody},
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

fn cpu_thresholds(
    limit: Option<ThermodynamicTemperature>,
    overrides: ThresholdOverrides,
) -> TemperatureThresholds {
    let from_hardware = match limit {
        Some(throttle) => TemperatureThresholds {
            warning: throttle - margin(WARNING_MARGIN_CELSIUS),
            critical: throttle - margin(CRITICAL_MARGIN_CELSIUS),
        },
        None => TemperatureThresholds {
            warning: celsius(UNREPORTED_WARNING_CELSIUS),
            critical: celsius(UNREPORTED_CRITICAL_CELSIUS),
        },
    };
    from_hardware.overridden_by(overrides)
}

impl PerCore for CoreTemperature {
    fn core(&self) -> u32 {
        self.core
    }
}

impl CoreTemperature {
    fn table_record(&self) -> TableRecord<2> {
        TableRecord::new(
            format!("Core {}", self.core),
            [
                Some(celsius_text(self.temperature).into()),
                self.limit.map(|limit| celsius_text(limit).into()),
            ],
        )
        .level(cpu_thresholds(self.limit, self.overrides).level(self.temperature))
    }
}

impl Temperature {
    fn thresholds(&self) -> TemperatureThresholds {
        cpu_thresholds(self.throttle_limit, self.overrides)
    }

    fn throttle_text(&self) -> Option<String> {
        self.throttle_limit.map(celsius_text)
    }
}

impl BarWidget for Temperature {
    fn name(&self) -> &'static str {
        "CPU temperature"
    }

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
        let thresholds = self.thresholds();
        let summary = Section::untitled_dictionary(
            [
                Some(KeyValue::new("Chips", chips.join(", "))),
                Some(KeyValue::new("Samples", self.readings.len().to_string())),
                self.lowest()
                    .map(|lowest| KeyValue::new("Min", celsius_text(lowest))),
                Some(KeyValue::new("Avg", celsius_text(self.average()))),
                Some(KeyValue::new("Max", celsius_text(self.hottest()))),
                self.throttle_text()
                    .map(|throttle| KeyValue::new("Throttles at", throttle)),
                Some(KeyValue::new(
                    "Warns above",
                    celsius_text(thresholds.warning),
                )),
                Some(KeyValue::new(
                    "Critical above",
                    celsius_text(thresholds.critical),
                )),
            ]
            .into_iter()
            .flatten(),
        );
        TooltipBody::sections(&[summary])
    }
}

impl CpuCoreTemperatures {
    pub fn sections(&self, topology: &CpuTopology) -> Vec<Section> {
        sections_by_core_kind(
            topology,
            self.cores.clone(),
            ["Temp", "Limit"],
            CoreTemperature::table_record,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reported_limit_sets_thresholds_below_throttling() {
        let thresholds = cpu_thresholds(Some(celsius(100.0)), ThresholdOverrides::default());
        assert_eq!(thresholds.level(celsius(79.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(81.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(96.0)), Severity::Critical);
    }

    #[test]
    fn unreported_limit_falls_back_to_fixed_thresholds() {
        let thresholds = cpu_thresholds(None, ThresholdOverrides::default());
        assert_eq!(thresholds.level(celsius(69.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(71.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(86.0)), Severity::Critical);
    }
}
