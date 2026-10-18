use sottomano_config::severity::Severity;
use sottomano_sources::{
    disk::{
        btrfs::DeviceErrors,
        health::{DriveHealth, DriveTemperature, MissingSensor},
    },
    temperature_thresholds::celsius_text,
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
    phosphor::Phosphor,
    tooltip_body::{KeyValue, Section, TooltipBody},
    widget_state::WidgetState,
};

const ERROR_RESET_COMMAND: &str = "btrfs device stats -z /";

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
