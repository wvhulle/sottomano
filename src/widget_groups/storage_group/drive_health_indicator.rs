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
        tooltip_body::{Row, Section, TooltipBody, row, section, untitled},
        widget_state::WidgetState,
    },
};

const ERROR_RESET_COMMAND: &str = "btrfs device stats -z /";
const UNREPORTED_WARNING_CELSIUS: f64 = 60.0;
const UNREPORTED_CRITICAL_CELSIUS: f64 = 70.0;

impl BarWidget for DriveHealth {
    fn icon(&self) -> Glyph {
        Phosphor::HardDrives(self.state().weight()).into()
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
        let drive = untitled(
            [row("Drive", self.model.clone())]
                .into_iter()
                .chain(self.temperature_rows())
                .collect(),
        );
        TooltipBody::Sections(vec![drive, self.errors.section()])
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

    fn temperature_rows(&self) -> Vec<Row> {
        match self.temperature {
            DriveTemperature::Reported { current, .. } => {
                let thresholds = self.temperature_thresholds();
                vec![
                    row("Temperature", celsius_text(current)),
                    row("Warns above", celsius_text(thresholds.warning)),
                    row("Critical above", celsius_text(thresholds.critical)),
                ]
            }
            DriveTemperature::Unreported(missing) => {
                vec![row("Temperature", missing.explanation())]
            }
        }
    }
}

impl DriveTemperature {
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
            ErrorTracking::Tracked(errors) => section("I/O errors", error_rows(errors)),
            ErrorTracking::NotTracked => {
                section("I/O errors", vec![row("Counters", "not tracked")])
            }
        }
    }
}

fn error_rows(errors: &DeviceErrors) -> Vec<Row> {
    if errors.total() == 0 {
        return vec![row("Since reset", "none")];
    }
    errors
        .by_kind()
        .into_iter()
        .filter(|error| error.count > 0)
        .map(|error| row(error.kind, error.count.to_string()))
        .chain([row("Reset with", ERROR_RESET_COMMAND)])
        .collect()
}
