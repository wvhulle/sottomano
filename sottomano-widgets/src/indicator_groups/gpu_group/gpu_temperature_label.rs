use sottomano_desktop_sources::cpu_memory_compute::{
    pci_gpu_samples::GpuTemperatureLimits, temperature_thresholds::celsius_text,
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
        ongoing_toast::{IndicatorAlerts, StateToast, ToastBody, ToastProgress},
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
                body: ToastBody::Text(label),
                progress: ToastProgress::Unmeasured,
            },
            ..IndicatorAlerts::default()
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    fn display_state(&self) -> IndicatorState {
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
