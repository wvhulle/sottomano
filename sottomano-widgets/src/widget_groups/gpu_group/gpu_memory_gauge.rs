use bytesize::ByteSize;
use chrono::Locale;
use sottomano_sources::cpu_memory_compute::pci_gpu_readings::{
    GpuMemory, GpuProcesses, SharedGpuMemory,
};
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
            GpuMemory::SharedWithSystem(shared) => {
                Some(shared.allocated.display().iec_short().to_string())
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
            GpuMemory::SharedWithSystem(shared) => shared_memory_sections(shared),
        }
    }
}

fn size_text(size: ByteSize) -> String {
    size.display().iec().to_string()
}

fn shared_memory_sections(shared: &SharedGpuMemory) -> Vec<Section> {
    let buffers = Section::dictionary([
        KeyValue::new("Allocated", size_text(shared.allocated)),
        KeyValue::new("Resident", size_text(shared.resident)),
    ])
    .titled("GPU buffers");
    let system = Section::dictionary(shared.system.as_ref().map(|system| {
        KeyValue::new(
            "Used",
            format!("{} of {}", size_text(system.used), size_text(system.total)),
        )
    }))
    .titled("System memory");
    vec![buffers, system]
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
