use sottomano_desktop_sources::cpu_memory_compute::{
    pci_gpu_readings::GpuTemperatureLimits, temperature_thresholds::celsius_text,
};
use sottomano_toml_config::severity::Severity;
use uom::si::f64::ThermodynamicTemperature;

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::{ReadingAlerts, StateToast, ToastBody, ToastProgress},
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct GpuTemperatureLabel {
    pub current: ThermodynamicTemperature,
    pub limits: GpuTemperatureLimits,
}

impl GpuTemperatureLabel {
    pub fn reading_row(self) -> ReadingRow<'static> {
        let label = celsius_text(self.current);
        let state = self.display_state();
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(label.clone()),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            alerts: ReadingAlerts {
                state_toast: StateToast::NotifyFromSeverity {
                    minimum_severity: Severity::Critical,
                    summary_text: "GPU overheating",
                    body: ToastBody::Text(label),
                    progress: ToastProgress::Unmeasured,
                },
                ..ReadingAlerts::default()
            },
            ..ReadingRow::glyph_only(
                "GPU temperature",
                Phosphor::ThermometerSimple(Weight::Regular).into(),
                state,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    fn display_state(&self) -> ReadingDisplayState {
        let reached = |limit: Option<ThermodynamicTemperature>| {
            limit.is_some_and(|limit| self.current >= limit)
        };
        if reached(self.limits.slowdown) {
            Severity::Critical.into()
        } else if reached(self.limits.target_max) {
            Severity::Warning.into()
        } else {
            Severity::Normal.into()
        }
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![
            Section::new(
                "Now",
                dictionary_table([LabeledValue::new(
                    "GPU",
                    Quantity::temperature(self.current),
                )]),
            ),
            Section::new(
                "Thresholds",
                dictionary_table(
                    [
                        self.limits.target_max.map(|target| {
                            LabeledValue::new("Target maximum", Quantity::temperature(target))
                        }),
                        self.limits.slowdown.map(|slowdown| {
                            LabeledValue::new("Slows down at", Quantity::temperature(slowdown))
                        }),
                    ]
                    .into_iter()
                    .flatten(),
                ),
            ),
        ]
    }
}
