use uom::si::f64::ThermodynamicTemperature;

use crate::{
    data::{
        severity::Severity,
        source::disk::{
            btrfs::DeviceErrors,
            health::{DriveHealth, DriveTemperature, MissingSensor},
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        hardware_section::HardwareEntries,
        phosphor::Phosphor,
        temperature_thresholds::{TemperatureThresholds, celsius, celsius_text},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const ERROR_RESET_COMMAND: &str = "btrfs device stats -z /";
const UNREPORTED_WARNING_CELSIUS: f64 = 60.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 70.0;

impl BarWidget for DriveHealth {
    fn name(&self) -> &'static str {
        "Drive health"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Thermometer(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        let error_level = if self.errors.is_some_and(|errors| errors.total() > 0) {
            Severity::Critical
        } else {
            Severity::Normal
        };
        self.temperature_level().max(error_level).into()
    }

    fn label(&self) -> Option<String> {
        self.temperature.current().map(celsius_text)
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Drive needs attention",
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let thresholds = self
            .temperature
            .current()
            .map(|_| self.temperature_thresholds());
        let summary = Section::dictionary(
            [
                KeyValue::new("Drive", self.device_name.as_str()),
                KeyValue::new("Temperature", self.temperature.text()),
            ]
            .into_iter()
            .chain(thresholds.into_iter().flat_map(|thresholds| {
                [
                    KeyValue::new("Warns above", celsius_text(thresholds.warning)),
                    KeyValue::new("Critical above", celsius_text(thresholds.critical)),
                ]
            })),
        );
        TooltipBody::sections([
            summary,
            self.identity.hardware_section(),
            Section::dictionary(self.errors.iter().flat_map(error_entries)).titled("I/O errors"),
        ])
    }
}

impl DriveHealth {
    fn temperature_level(&self) -> Severity {
        match self.temperature {
            DriveTemperature::Reported { current, .. } => {
                self.temperature_thresholds().level(current)
            }
            DriveTemperature::Unreported(_) => Severity::Normal,
        }
    }

    fn temperature_thresholds(&self) -> TemperatureThresholds {
        TemperatureThresholds {
            warning: self
                .temperature
                .warning()
                .unwrap_or_else(|| celsius(UNREPORTED_WARNING_CELSIUS)),
            critical: self
                .temperature
                .critical()
                .unwrap_or_else(|| celsius(UNREPORTED_CRITICAL_CELSIUS)),
        }
        .overridden_by(self.overrides)
    }
}

impl DriveTemperature {
    fn text(self) -> String {
        match self {
            DriveTemperature::Reported { current, .. } => celsius_text(current),
            DriveTemperature::Unreported(missing) => missing.explanation().to_owned(),
        }
    }

    fn current(&self) -> Option<ThermodynamicTemperature> {
        match self {
            DriveTemperature::Reported { current, .. } => Some(*current),
            DriveTemperature::Unreported(_) => None,
        }
    }

    fn warning(&self) -> Option<ThermodynamicTemperature> {
        match self {
            DriveTemperature::Reported { warning, .. } => *warning,
            DriveTemperature::Unreported(_) => None,
        }
    }

    fn critical(&self) -> Option<ThermodynamicTemperature> {
        match self {
            DriveTemperature::Reported { critical, .. } => *critical,
            DriveTemperature::Unreported(_) => None,
        }
    }
}

impl MissingSensor {
    fn explanation(self) -> &'static str {
        match self {
            MissingSensor::DrivetempNotLoaded => "load the drivetemp kernel module",
            MissingSensor::NotExposed => "not reported by the drive",
        }
    }
}

fn error_entries(errors: &DeviceErrors) -> Vec<KeyValue> {
    if errors.total() == 0 {
        return vec![KeyValue::new("Since reset", "none")];
    }
    errors
        .by_kind()
        .into_iter()
        .filter(|error| error.count > 0)
        .map(|error| KeyValue::new(error.kind, error.count.to_string()))
        .chain([KeyValue::new("Reset with", ERROR_RESET_COMMAND)])
        .collect()
}
