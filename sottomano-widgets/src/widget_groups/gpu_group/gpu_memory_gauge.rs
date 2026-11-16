use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_samples::{
    GpuMemory, GpuProcesses, SharedGpuMemory,
};

use crate::{
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub fn gpu_memory_indicator<'indicator>(memory: GpuMemory) -> GroupIndicator<'indicator> {
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
    GroupIndicator::glyph_only(
        name,
        Phosphor::Memory(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_readout(IndicatorReadout {
        label,
        gauge,
        badge: ReadoutBadge::Unbadged,
    })
    .with_hover_card(Box::new(move |_| gpu_memory_sections(&memory)))
}

pub fn gpu_memory_sections(gpu_memory: &GpuMemory) -> Vec<Section> {
    match gpu_memory {
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

pub fn gpu_processes_sections(gpu_processes: &GpuProcesses) -> Vec<Section> {
    vec![Section::new(
        "Processes",
        dictionary_table(gpu_processes.processes.iter().filter_map(|process| {
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
