use sottomano_desktop_sources::cpu_memory_compute::{
    pci_gpu_samples::{GpuTemperatureLimits, GpuTemperatureThreshold},
    temperature_thresholds::celsius_text,
};
use sottomano_toml_config::severity::Severity;
use uom::si::f64::ThermodynamicTemperature;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::{IndicatorAlerts, NotificationBody, NotificationProgress, StateToast},
        value_cells::LabeledValue,
    },
};

#[derive(Clone, Copy)]
pub struct GpuTemperatureLabel {
    pub current: ThermodynamicTemperature,
    pub limits: GpuTemperatureLimits,
}

impl GpuTemperatureLabel {
    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        let label = celsius_text(self.current);
        let state = self.display_state();
        GroupIndicator::glyph_only(
            "GPU temperature",
            Phosphor::ThermometerSimple(Weight::Regular).into(),
            state,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(label.clone()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_alerts(IndicatorAlerts {
            state_toast: StateToast::NotifyFromSeverity {
                minimum_severity: Severity::Critical,
                summary_text: "GPU overheating",
                body: NotificationBody::Text(label),
                progress: NotificationProgress::Unmeasured,
            },
            ..IndicatorAlerts::default()
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn display_state(&self) -> IndicatorState {
        let reached = |threshold: GpuTemperatureThreshold| matches!(threshold, GpuTemperatureThreshold::Reported(limit) if self.current >= limit);
        if reached(self.limits.slowdown) {
            Severity::Critical.into()
        } else if reached(self.limits.target_max) {
            Severity::Warning.into()
        } else {
            Severity::Normal.into()
        }
    }

    pub fn hover_card(&self) -> Vec<Section> {
        let mut threshold_rows = Vec::new();
        if let GpuTemperatureThreshold::Reported(target) = self.limits.target_max {
            threshold_rows.push(LabeledValue::new(
                "Target maximum",
                Quantity::temperature(target),
            ));
        }
        if let GpuTemperatureThreshold::Reported(slowdown) = self.limits.slowdown {
            threshold_rows.push(LabeledValue::new(
                "Slows down at",
                Quantity::temperature(slowdown),
            ));
        }
        vec![
            Section::new(
                "Now",
                dictionary_table([LabeledValue::new(
                    "GPU",
                    Quantity::temperature(self.current),
                )]),
            ),
            Section::new("Thresholds", dictionary_table(threshold_rows)),
        ]
    }
}
