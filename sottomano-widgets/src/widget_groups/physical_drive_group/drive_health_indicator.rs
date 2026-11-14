use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::{
    SnapshotSource,
    cpu_memory_compute::temperature_thresholds::celsius_text,
    disk_storage::{
        drive_temperature_health::{DriveHealth, DriveTemperature, MissingSensor},
        filesystem_device_errors::DeviceErrors,
    },
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

impl GroupItemReading for DriveHealth {
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Thermometer(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        let error_level = if self.errors.iter().any(|errors| errors.total() > 0) {
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

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
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
        HoverCardBody::sections(
            [summary, threshold_section, Section::from(&self.identity)]
                .into_iter()
                .chain(self.errors.iter().map(|errors| {
                    Section::dictionary(error_entries(errors))
                        .titled(format!("I/O errors on {}", errors.counted_on))
                })),
        )
    }
}

fn error_entries(errors: &DeviceErrors) -> Vec<LabeledValue> {
    if errors.total() == 0 {
        return vec![LabeledValue::new("Since reset", "none")];
    }
    errors
        .nonzero_counts()
        .map(|error| LabeledValue::new(error.kind, Quantity::count(error.count)))
        .chain([LabeledValue::new(
            "Reset with",
            errors.reset_command.clone(),
        )])
        .collect()
}
