use uom::si::f64::ThermodynamicTemperature;

use crate::{
    data::{
        severity::Severity,
        source::{
            disk::{
                btrfs::DeviceErrors,
                health::{DriveHealth, DriveTemperature, ErrorTracking, MissingSensor},
            },
            sysfs_sensors::temperature::ThermalLimit,
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        phosphor::Phosphor,
        temperature_thresholds::{TemperatureThresholds, celsius_text, reported_or},
        tooltip_body::{Section, TooltipBody},
        widget_state::WidgetState,
    },
};

const ERROR_RESET_COMMAND: &str = "btrfs device stats -z /";
const UNREPORTED_WARNING_CELSIUS: f64 = 60.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 70.0;

impl BarWidget for DriveHealth {
    fn icon(&self) -> Glyph {
        Phosphor::Thermometer(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        self.temperature_level().max(self.errors.level()).into()
    }

    fn label(&self) -> Option<String> {
        self.temperature.current().map(celsius_text)
    }

    fn reading_alert_threshold(&self) -> ReadingAlertThreshold {
        ReadingAlertThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Drive needs attention",
        }
    }

    fn tooltip(&self) -> TooltipBody {
        let thresholds = self
            .temperature
            .current()
            .map(|_| self.temperature_thresholds());
        TooltipBody::sections(&[
            Section::untitled(&[
                ("Drive", Some(self.device_name.clone())),
                ("Temperature", Some(self.temperature.text())),
                (
                    "Warns above",
                    thresholds.map(|thresholds| celsius_text(thresholds.warning)),
                ),
                (
                    "Critical above",
                    thresholds.map(|thresholds| celsius_text(thresholds.critical)),
                ),
            ]),
            self.errors.section(),
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
            warning: reported_or(self.temperature.warning(), UNREPORTED_WARNING_CELSIUS),
            critical: reported_or(self.temperature.critical(), UNREPORTED_CRITICAL_CELSIUS),
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

    fn warning(&self) -> ThermalLimit {
        match self {
            DriveTemperature::Reported { warning, .. } => *warning,
            DriveTemperature::Unreported(_) => ThermalLimit::Unreported,
        }
    }

    fn critical(&self) -> ThermalLimit {
        match self {
            DriveTemperature::Reported { critical, .. } => *critical,
            DriveTemperature::Unreported(_) => ThermalLimit::Unreported,
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

impl ErrorTracking {
    fn level(&self) -> Severity {
        match self {
            ErrorTracking::Tracked(errors) if errors.total() > 0 => Severity::Critical,
            ErrorTracking::Tracked(_) | ErrorTracking::NotTracked => Severity::Normal,
        }
    }

    fn section(&self) -> Section {
        match self {
            ErrorTracking::Tracked(errors) => Section::titled("I/O errors", &error_rows(errors)),
            ErrorTracking::NotTracked => {
                Section::titled("I/O errors", &[("Counters", "not tracked")])
            }
        }
    }
}

fn error_rows(errors: &DeviceErrors) -> Vec<(&'static str, String)> {
    if errors.total() == 0 {
        return vec![("Since reset", "none".to_owned())];
    }
    errors
        .by_kind()
        .into_iter()
        .filter(|error| error.count > 0)
        .map(|error| (error.kind, error.count.to_string()))
        .chain([("Reset with", ERROR_RESET_COMMAND.to_owned())])
        .collect()
}
