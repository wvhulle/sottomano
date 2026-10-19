use sottomano_sources::cpu_memory_compute::nvml_gpu_readings::{GpuProcesses, VramUsage};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_text,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, ReadingSections, Section, TooltipBody},
    },
};

impl BarWidget for VramUsage {
    fn name(&self) -> &'static str {
        "VRAM"
    }

    fn icon(&self) -> Glyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(self.fill())
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        TooltipBody::sections(self.sections(clock))
    }
}

impl ReadingSections for VramUsage {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::dictionary([
            KeyValue::new("VRAM", percent_text(self.fill())),
            KeyValue::new("Used", self.used.display().iec().to_string()),
            KeyValue::new("Total", self.total.display().iec().to_string()),
        ])]
    }
}

impl ReadingSections for GpuProcesses {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![
            Section::dictionary(self.processes.iter().filter_map(|process| {
                Some(KeyValue::new(
                    process
                        .name
                        .clone()
                        .unwrap_or_else(|| process.pid.to_string()),
                    process.vram?.display().iec().to_string(),
                ))
            }))
            .titled("Processes"),
        ]
    }
}
