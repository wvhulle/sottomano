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
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::{ReadingAlerts, StateToast, ToastBody, ToastProgress},
        phosphor::{Phosphor, Weight},
        value_cells::{LabeledValue, ValueCell},
    },
};

pub(crate) fn drive_health_reading_row(
    health: &DriveHealth,
    _clock: Clock,
    _locale: Locale,
) -> ReadingRow<'_> {
    let error_level = if health.errors.iter().any(|errors| errors.total() > 0) {
        Severity::Critical
    } else {
        Severity::Normal
    };
    let temperature_text = health.temperature.current().map(celsius_text);
    let (label, body) = match temperature_text {
        Some(text) => (ReadoutLabel::Labeled(text.clone()), ToastBody::Text(text)),
        None => (ReadoutLabel::Unlabeled, ToastBody::Bodiless),
    };
    ReadingRow {
        readout: RowReadout {
            label,
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        alerts: ReadingAlerts {
            state_toast: StateToast::NotifyFromSeverity {
                minimum_severity: Severity::Critical,
                summary_text: "Drive needs attention",
                body,
                progress: ToastProgress::Unmeasured,
            },
            ..ReadingAlerts::default()
        },
        ..ReadingRow::glyph_only(
            DriveHealth::NAME,
            Phosphor::Thermometer(Weight::Regular).into(),
            health.temperature_level().max(error_level).into(),
            Box::new(move |card_clock| drive_health_hover_card(health, card_clock)),
        )
    }
}

pub(crate) fn drive_health_hover_card(health: &DriveHealth, _clock: Clock) -> Vec<Section> {
    let thresholds = health
        .temperature
        .current()
        .map(|_| health.temperature_thresholds());
    let temperature_cell = match health.temperature {
        DriveTemperature::Reported { current, .. } => Quantity::temperature(current).into(),
        DriveTemperature::Unreported(MissingSensor::DrivetempNotLoaded) => {
            ValueCell::from("load the drivetemp kernel module")
        }
        DriveTemperature::Unreported(MissingSensor::NotExposed) => {
            ValueCell::from("not reported by the drive")
        }
    };
    let summary = Section::new(
        "Temperature",
        dictionary_table([
            LabeledValue::new("Drive", health.device_name.as_str()),
            LabeledValue::new("Temperature", temperature_cell),
        ]),
    );
    let threshold_section = Section::new(
        "Thresholds",
        dictionary_table(thresholds.into_iter().flat_map(|thresholds| {
            [
                LabeledValue::new("Warns above", Quantity::temperature(thresholds.warning)),
                LabeledValue::new("Critical above", thresholds.critical_text()),
            ]
        })),
    );
    [summary, threshold_section, Section::from(health)]
        .into_iter()
        .chain(health.errors.iter().map(|errors| {
            Section::new(
                format!("I/O errors on {}", errors.counted_on),
                dictionary_table(error_entries(errors)),
            )
        }))
        .collect()
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
