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
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
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
        let temperature_text = match self.temperature {
            DriveTemperature::Reported { current, .. } => celsius_text(current),
            DriveTemperature::Unreported(MissingSensor::DrivetempNotLoaded) => {
                "load the drivetemp kernel module".to_owned()
            }
            DriveTemperature::Unreported(MissingSensor::NotExposed) => {
                "not reported by the drive".to_owned()
            }
        };
        let summary = Section::dictionary([
            KeyValue::new("Drive", self.device_name.as_str()),
            KeyValue::new("Temperature", temperature_text),
        ]);
        let threshold_section =
            Section::dictionary(thresholds.into_iter().flat_map(|thresholds| {
                [
                    KeyValue::new("Warns above", celsius_text(thresholds.warning)),
                    KeyValue::new("Critical above", thresholds.critical_text()),
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
