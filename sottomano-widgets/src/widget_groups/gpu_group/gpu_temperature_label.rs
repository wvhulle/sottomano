use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::cpu_memory_compute::{
    pci_gpu_readings::GpuTemperatureLimits, temperature_thresholds::celsius_text,
};
use uom::si::f64::ThermodynamicTemperature;

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
        value_cells::LabeledValue,
    },
};

pub struct GpuTemperatureLabel {
    pub current: ThermodynamicTemperature,
    pub limits: GpuTemperatureLimits,
}

impl GroupItemReading for GpuTemperatureLabel {
    fn name(&self) -> &'static str {
        "GPU temperature"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ThermometerSimple(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
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

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(celsius_text(self.current))
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "GPU overheating",
        }
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([
            Section::dictionary([LabeledValue::new(
                "GPU",
                Quantity::temperature(self.current),
            )]),
            Section::dictionary(
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
            )
            .titled("Thresholds"),
        ])
    }
}
