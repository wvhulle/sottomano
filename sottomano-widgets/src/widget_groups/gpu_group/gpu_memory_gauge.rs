use chrono::Locale;
use sottomano_sources::cpu_memory_compute::pci_gpu_readings::{GpuMemory, GpuProcesses};
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

impl BarWidget for GpuMemory {
    fn name(&self) -> &'static str {
        match self {
            GpuMemory::Dedicated(_) => "VRAM",
            GpuMemory::SharedWithSystem { .. } => "GPU memory",
        }
    }

    fn icon(&self) -> Glyph {
        Phosphor::Memory(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        match self {
            GpuMemory::Dedicated(vram) => Some(vram.fill()),
            GpuMemory::SharedWithSystem { .. } => None,
        }
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        match self {
            GpuMemory::Dedicated(_) => None,
            GpuMemory::SharedWithSystem { resident } => {
                Some(resident.display().iec_short().to_string())
            }
        }
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        TooltipBody::sections(self.sections(clock))
    }
}

impl ReadingSections for GpuMemory {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        match self {
            GpuMemory::Dedicated(vram) => vec![Section::dictionary([
                KeyValue::new("VRAM", percent_text(vram.fill())),
                KeyValue::new("Used", vram.used.display().iec().to_string()),
                KeyValue::new("Total", vram.total.display().iec().to_string()),
            ])],
            GpuMemory::SharedWithSystem { resident } => vec![Section::dictionary([
                KeyValue::new("Resident", resident.display().iec().to_string()),
                KeyValue::new("Location", "system memory"),
            ])],
        }
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
                    process.resident_memory?.display().iec().to_string(),
                ))
            }))
            .titled("Processes"),
        ]
    }
}
