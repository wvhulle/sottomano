use sottomano_sources::cpu_memory_compute::nvml_gpu_readings::{GpuClocks, GpuSnapshot};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{
        f64::{Frequency, Ratio},
        frequency::megahertz,
    },
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_text,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, ReadingSections, Section, TooltipBody},
    },
};

impl BarWidget for GpuSnapshot {
    fn name(&self) -> &'static str {
        "GPU load"
    }

    fn icon(&self) -> Glyph {
        Phosphor::GraphicsCard(Weight::Regular).into()
    }

    fn fallback_icon() -> Glyph {
        Phosphor::GraphicsCard(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.load)
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([KeyValue::new("GPU", percent_text(self.load))])
    }
}

impl ReadingSections for GpuClocks {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let clock_text = |key: &'static str, clock: Option<Frequency>| {
            clock.map(|frequency| {
                KeyValue::new(
                    key,
                    format!("{:.0}", frequency.into_format_args(megahertz, Abbreviation)),
                )
            })
        };
        vec![Section::dictionary(
            [
                self.performance_state
                    .map(|state| KeyValue::new("Performance state", format!("P{state}"))),
                clock_text("Graphics clock", self.graphics_clock),
                clock_text("Memory clock", self.memory_clock),
                self.fan
                    .map(|speed| KeyValue::new("Fan", percent_text(speed))),
            ]
            .into_iter()
            .flatten(),
        )]
    }
}
