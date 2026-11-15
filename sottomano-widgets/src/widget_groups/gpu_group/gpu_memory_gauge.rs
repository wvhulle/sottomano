use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_readings::{
    GpuMemory, GpuProcesses, SharedGpuMemory,
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{ReadingSections, Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub fn gpu_memory_reading_row(memory: GpuMemory) -> ReadingRow<'static> {
    let name = match &memory {
        GpuMemory::Dedicated(_) => "VRAM",
        GpuMemory::SharedWithSystem { .. } => "GPU memory",
    };
    let (label, gauge) = match &memory {
        GpuMemory::Dedicated(vram) => (
            ReadoutLabel::Unlabeled,
            ReadoutGauge::Gauged {
                fill: vram.fill(),
                live: vram.fill(),
            },
        ),
        GpuMemory::SharedWithSystem(shared) => (
            ReadoutLabel::Labeled(shared.allocated.display().iec_short().to_string()),
            ReadoutGauge::Ungauged,
        ),
    };
    ReadingRow {
        readout: RowReadout {
            label,
            gauge,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            name,
            Phosphor::Memory(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(move |clock| memory.sections(clock)),
        )
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
