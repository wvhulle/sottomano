use uom::si::f64::ThermodynamicTemperature;

use crate::{
    data::{severity::Severity, source::gpu::GpuTemperatureLimits},
    widgets::{
        bar_widget::{BarWidget, Glyph, OngoingToastThreshold},
        phosphor::Phosphor,
        temperature_thresholds::celsius_text,
        tooltip_body::{KeyValue, TooltipBody},
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
        Phosphor::ThermometerSimple(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
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

    fn label(&self) -> Option<String> {
        Some(celsius_text(self.current))
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "GPU overheating",
        }
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::dictionary(
            [
                Some(KeyValue::new("GPU", celsius_text(self.current))),
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
    }
}
