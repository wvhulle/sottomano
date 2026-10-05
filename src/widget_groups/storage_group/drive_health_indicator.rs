use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius},
};

use crate::{
    data::{
        severity::Severity,
        source::{
            disk::{
                btrfs::DeviceErrors,
                health::{DriveHealth, DriveTemperature, ErrorTracking},
            },
            sysfs_sensors::temperature::ThermalLimit,
        },
    },
    widgets::{
        bar_widget::{BarWidget, Glyph, ReadingAlertThreshold},
        phosphor::Phosphor,
        tooltip_body::{Row, Section, TooltipBody, row, section, untitled},
        widget_state::WidgetState,
    },
};

const ERROR_RESET_COMMAND: &str = "btrfs device stats -z /";

fn celsius_text(temperature: ThermodynamicTemperature) -> String {
    format!(
        "{:.0}",
        temperature.into_format_args(degree_celsius, Abbreviation)
    )
}

impl BarWidget for DriveHealth {
    fn icon(&self) -> Glyph {
        Phosphor::HardDrives(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        self.temperature.level().max(self.errors.level()).into()
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
                .chain(self.temperature.rows())
                .collect(),
        );
        TooltipBody::Sections(vec![drive, self.errors.section()])
    }
}

impl DriveTemperature {
    fn current(&self) -> Option<ThermodynamicTemperature> {
        match self {
            DriveTemperature::Reported { current, .. } => Some(*current),
            DriveTemperature::Unreported => None,
        }
    }

    fn level(&self) -> Severity {
        match self {
            DriveTemperature::Reported {
                current,
                warning,
                critical,
            } => {
                if reached(*current, *critical) {
                    Severity::Critical
                } else if reached(*current, *warning) {
                    Severity::Warning
                } else {
                    Severity::Normal
                }
            }
            DriveTemperature::Unreported => Severity::Normal,
        }
    }

    fn rows(&self) -> Vec<Row> {
        match self {
            DriveTemperature::Reported {
                current,
                warning,
                critical,
            } => [
                Some(row("Temperature", celsius_text(*current))),
                limit_row("Warns at", *warning),
                limit_row("Critical at", *critical),
            ]
            .into_iter()
            .flatten()
            .collect(),
            DriveTemperature::Unreported => vec![row("Temperature", "not reported")],
        }
    }
}

fn reached(current: ThermodynamicTemperature, limit: ThermalLimit) -> bool {
    match limit {
        ThermalLimit::Reported(limit) => current >= limit,
        ThermalLimit::Unreported => false,
    }
}

fn limit_row(key: &str, limit: ThermalLimit) -> Option<Row> {
    match limit {
        ThermalLimit::Reported(limit) => Some(row(key, celsius_text(limit))),
        ThermalLimit::Unreported => None,
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
