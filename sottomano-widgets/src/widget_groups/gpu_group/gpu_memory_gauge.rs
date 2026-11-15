use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_readings::{
    GpuMemory, GpuProcesses, SharedGpuMemory,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{ReadingSections, Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for GpuMemory {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            GpuMemory::Dedicated(_) => "VRAM",
            GpuMemory::SharedWithSystem { .. } => "GPU memory",
        })
    }

    fn icon(&self) -> IconGlyph {
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

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.sections(clock).into_iter().collect::<Vec<_>>()
    }
}

impl ReadingSections for GpuMemory {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        match self {
            GpuMemory::Dedicated(vram) => vec![Section::new(
                "Usage",
                dictionary_table([
                    LabeledValue::new("VRAM", Quantity::share(vram.fill())),
                    LabeledValue::new("Used", Quantity::size(vram.used)),
                    LabeledValue::new("Total", Quantity::size(vram.total)),
                ]),
            )],
            GpuMemory::SharedWithSystem(shared) => shared_memory_sections(shared),
        }
    }
}

fn shared_memory_sections(shared: &SharedGpuMemory) -> Vec<Section> {
    let buffers = Section::new(
        "GPU buffers",
        dictionary_table([
            LabeledValue::new("Allocated", Quantity::size(shared.allocated)),
            LabeledValue::new("Resident", Quantity::size(shared.resident)),
        ]),
    );
    let system = Section::new(
        "System memory",
        dictionary_table(shared.system.as_ref().map(|system| {
            LabeledValue::new(
                "Used",
                format!(
                    "{} of {}",
                    system.used.display().iec(),
                    system.total.display().iec()
                ),
            )
        })),
    );
    vec![buffers, system]
}

impl ReadingSections for GpuProcesses {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        vec![Section::new(
            "Processes",
            dictionary_table(self.processes.iter().filter_map(|process| {
                Some(LabeledValue::new(
                    process
                        .name
                        .clone()
                        .unwrap_or_else(|| process.pid.to_string()),
                    Quantity::size(process.resident_memory?),
                ))
            })),
        )]
    }
}
