use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::{
    cpu_memory_compute::temperature_thresholds::celsius_text,
    disk_storage::{
        btrfs_filesystem_space::DeviceErrors,
        drive_temperature_health::{DriveHealth, DriveTemperature, MissingSensor},
    },
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::{LabeledValue, ValueCell},
        widget_state::WidgetState,
    },
};

const ERROR_RESET_COMMAND: &str = "btrfs device stats -z /";

impl BarWidget for DriveHealth {
    fn name(&self) -> &'static str {
        "Drive health"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Thermometer(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
        let error_level = if self.errors.is_some_and(|errors| errors.total() > 0) {
            Severity::Critical
        } else {
            Severity::Normal
        };
        self.temperature_level().max(error_level).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        self.temperature.current().map(celsius_text)
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Drive needs attention",
        }
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let thresholds = self
            .temperature
            .current()
            .map(|_| self.temperature_thresholds());
        let temperature_cell = match self.temperature {
            DriveTemperature::Reported { current, .. } => Quantity::temperature(current).into(),
            DriveTemperature::Unreported(MissingSensor::DrivetempNotLoaded) => {
                ValueCell::from("load the drivetemp kernel module")
            }
            DriveTemperature::Unreported(MissingSensor::NotExposed) => {
                ValueCell::from("not reported by the drive")
            }
        };
        let summary = Section::dictionary([
            LabeledValue::new("Drive", self.device_name.as_str()),
            LabeledValue::new("Temperature", temperature_cell),
        ]);
        let threshold_section =
            Section::dictionary(thresholds.into_iter().flat_map(|thresholds| {
                [
                    LabeledValue::new("Warns above", Quantity::temperature(thresholds.warning)),
                    LabeledValue::new("Critical above", thresholds.critical_text()),
                ]
            }))
            .titled("Thresholds");
        TooltipBody::sections([
            summary,
            threshold_section,
            Section::from(&self.identity),
            Section::dictionary(self.errors.iter().flat_map(error_entries)).titled("I/O errors"),
        ])
    }
}

fn error_entries(errors: &DeviceErrors) -> Vec<LabeledValue> {
    if errors.total() == 0 {
        return vec![LabeledValue::new("Since reset", "none")];
    }
    errors
        .by_kind()
        .into_iter()
        .filter(|error| error.count > 0)
        .map(|error| LabeledValue::new(error.kind, Quantity::count(error.count)))
        .chain([LabeledValue::new("Reset with", ERROR_RESET_COMMAND)])
        .collect()
}
