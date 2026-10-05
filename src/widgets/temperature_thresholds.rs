use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius},
};

use crate::{
    config::temperature::{Threshold, ThresholdOverrides},
    data::{severity::Severity, source::sysfs_sensors::temperature::ThermalLimit},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemperatureThresholds {
    pub warning: ThermodynamicTemperature,
    pub critical: ThermodynamicTemperature,
}

impl TemperatureThresholds {
    pub fn overridden_by(self, overrides: ThresholdOverrides) -> Self {
        TemperatureThresholds {
            warning: configured_or(overrides.warning, self.warning),
            critical: configured_or(overrides.critical, self.critical),
        }
    }

    pub fn level(self, temperature: ThermodynamicTemperature) -> Severity {
        if temperature > self.critical {
            Severity::Critical
        } else if temperature > self.warning {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }
}

fn configured_or(
    threshold: Threshold,
    from_hardware: ThermodynamicTemperature,
) -> ThermodynamicTemperature {
    match threshold {
        Threshold::FromHardware => from_hardware,
        Threshold::Celsius(degrees) => celsius(f64::from(degrees)),
    }
}

pub fn reported_or(limit: ThermalLimit, fallback_celsius: f64) -> ThermodynamicTemperature {
    match limit {
        ThermalLimit::Reported(limit) => limit,
        ThermalLimit::Unreported => celsius(fallback_celsius),
    }
}

pub fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(degrees)
}

pub fn celsius_text(temperature: ThermodynamicTemperature) -> String {
    format!(
        "{:.0}",
        temperature.into_format_args(degree_celsius, Abbreviation)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_thresholds_replace_hardware_ones_independently() {
        let from_hardware = TemperatureThresholds {
            warning: celsius(80.0),
            critical: celsius(95.0),
        };
        let thresholds = from_hardware.overridden_by(ThresholdOverrides {
            warning: Threshold::FromHardware,
            critical: Threshold::Celsius(85),
        });
        assert_eq!(thresholds.level(celsius(79.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(81.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(86.0)), Severity::Critical);
    }
}
