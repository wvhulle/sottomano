use chrono::Locale;
use sottomano_config::severity::Severity;
use sottomano_sources::cpu_memory_compute::{
    nvml_gpu_readings::GpuTemperatureLimits, temperature_thresholds::celsius_text,
};
use uom::si::f64::ThermodynamicTemperature;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, Section, TooltipBody},
        widget_state::WidgetState,
    },
};

pub struct GpuTemperatureLabel {
    pub current: ThermodynamicTemperature,
    pub limits: GpuTemperatureLimits,
}

impl BarWidget for GpuTemperatureLabel {
    fn name(&self) -> &'static str {
        "GPU temperature"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ThermometerSimple(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> WidgetState {
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

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::sections([
            Section::dictionary([KeyValue::new("GPU", celsius_text(self.current))]),
            Section::dictionary(
                [
                    self.limits
                        .target_max
                        .map(|target| KeyValue::new("Target maximum", celsius_text(target))),
                    self.limits
                        .slowdown
                        .map(|slowdown| KeyValue::new("Slows down at", celsius_text(slowdown))),
                ]
                .into_iter()
                .flatten(),
            )
            .titled("Thresholds"),
        ])
    }
}
