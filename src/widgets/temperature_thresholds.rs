use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{TemperatureInterval, ThermodynamicTemperature},
        temperature_interval,
        thermodynamic_temperature::{degree_celsius, kelvin},
    },
};

use crate::{
    config::widget_groups::temperature_sensors::{Threshold, ThresholdOverrides},
    data::severity::Severity,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThresholdBasis {
    BelowThrottle(ThermodynamicTemperature),
    Reported,
    Assumed,
    Configured,
}

impl ThresholdBasis {
    pub fn text(self, critical: ThermodynamicTemperature) -> String {
        match self {
            ThresholdBasis::BelowThrottle(throttle) => format!(
                "{} below the {} throttle point",
                interval_text(TemperatureInterval::new::<temperature_interval::kelvin>(
                    throttle.get::<kelvin>() - critical.get::<kelvin>()
                )),
                celsius_text(throttle)
            ),
            ThresholdBasis::Reported => "reported by the sensor".into(),
            ThresholdBasis::Assumed => "assumed, no limit reported by the sensor".into(),
            ThresholdBasis::Configured => "from config".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemperatureThresholds {
    pub warning: ThermodynamicTemperature,
    pub critical: ThermodynamicTemperature,
    pub basis: ThresholdBasis,
}

impl TemperatureThresholds {
    pub fn overridden_by(self, overrides: ThresholdOverrides) -> Self {
        TemperatureThresholds {
            warning: configured_or(overrides.warning, self.warning),
            critical: configured_or(overrides.critical, self.critical),
            basis: match overrides.critical {
                Threshold::FromHardware => self.basis,
                Threshold::Celsius(_) => ThresholdBasis::Configured,
            },
        }
    }

    pub fn critical_text(self) -> String {
        format!(
            "{} ({})",
            celsius_text(self.critical),
            self.basis.text(self.critical)
        )
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

pub fn celsius(degrees: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(degrees)
}

pub fn celsius_text(temperature: ThermodynamicTemperature) -> String {
    format!(
        "{:.0}",
        temperature.into_format_args(degree_celsius, Abbreviation)
    )
}

fn interval_text(interval: TemperatureInterval) -> String {
    format!(
        "{:.0}",
        interval.into_format_args(temperature_interval::degree_celsius, Abbreviation)
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
            basis: ThresholdBasis::Reported,
        };
        let thresholds = from_hardware.overridden_by(ThresholdOverrides {
            warning: Threshold::FromHardware,
            critical: Threshold::Celsius(85),
        });
        assert_eq!(thresholds.basis, ThresholdBasis::Configured);
        assert_eq!(thresholds.level(celsius(79.0)), Severity::Normal);
        assert_eq!(thresholds.level(celsius(81.0)), Severity::Warning);
        assert_eq!(thresholds.level(celsius(86.0)), Severity::Critical);
    }
}
