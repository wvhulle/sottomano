use chrono::Locale;
use sottomano_sources::cpu_memory_compute::pci_gpu_readings::{
    GpuMemory, GpuProcesses, SharedGpuMemory,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        localized_quantities::Quantity,
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
                KeyValue::new("VRAM", Quantity::share(vram.fill())),
                KeyValue::new("Used", Quantity::size(vram.used)),
                KeyValue::new("Total", Quantity::size(vram.total)),
            ])],
            GpuMemory::SharedWithSystem(shared) => shared_memory_sections(shared),
        }
    }
}

fn shared_memory_sections(shared: &SharedGpuMemory) -> Vec<Section> {
    let buffers = Section::dictionary([
        KeyValue::new("Allocated", Quantity::size(shared.allocated)),
        KeyValue::new("Resident", Quantity::size(shared.resident)),
    ])
    .titled("GPU buffers");
    let system = Section::dictionary(shared.system.as_ref().map(|system| {
        KeyValue::new(
            "Used",
            format!(
                "{} of {}",
                system.used.display().iec(),
                system.total.display().iec()
            ),
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
                    Quantity::size(process.resident_memory?),
                ))
            }))
            .titled("Processes"),
        ]
    }
}
