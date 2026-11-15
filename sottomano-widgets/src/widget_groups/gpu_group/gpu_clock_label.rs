use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_readings::{
    GpuClocks, GpuThrottleReason,
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::megahertz},
};

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

fn megahertz_text(frequency: Frequency) -> String {
    format!("{:.0}", frequency.into_format_args(megahertz, Abbreviation))
}

#[derive(Clone, Copy)]
pub struct GpuClockLabel {
    pub current: Frequency,
}

impl GpuClockLabel {
    pub fn reading_row(self) -> ReadingRow<'static> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(megahertz_text(self.current)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                "GPU clock",
                Phosphor::WaveSine(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub fn hover_card(self) -> Vec<Section> {
        vec![Section::new(
            "Now",
            dictionary_table([LabeledValue::new(
                "Graphics clock",
                Quantity::clock_megahertz(self.current),
            )]),
        )]
    }
}

pub fn gpu_clocks_sections(gpu_clocks: &GpuClocks) -> Vec<Section> {
    let throttle_reasons = (!gpu_clocks.throttle_reasons.is_empty()).then(|| {
        Section::new(
            "Throttled by",
            dictionary_table(
                gpu_clocks
                    .throttle_reasons
                    .iter()
                    .map(|reason| LabeledValue::new(throttle_reason_text(*reason), "active")),
            ),
        )
    });
    [Section::new(
        "Performance",
        dictionary_table(
            [
                gpu_clocks.graphics_clock_ceiling.map(|ceiling| {
                    LabeledValue::new("Ceiling", Quantity::clock_megahertz(ceiling))
                }),
                gpu_clocks.memory_clock.map(|memory| {
                    LabeledValue::new("Memory clock", Quantity::clock_megahertz(memory))
                }),
                gpu_clocks
                    .performance_state
                    .map(|state| LabeledValue::new("Performance state", format!("P{state}"))),
                gpu_clocks
                    .fan
                    .map(|speed| LabeledValue::new("Fan", Quantity::share(speed))),
            ]
            .into_iter()
            .flatten(),
        ),
    )]
    .into_iter()
    .chain(throttle_reasons)
    .collect()
}

fn throttle_reason_text(reason: GpuThrottleReason) -> &'static str {
    match reason {
        GpuThrottleReason::SustainedPowerLimit => "Sustained power limit (PL1)",
        GpuThrottleReason::BurstPowerLimit => "Burst power limit (PL2)",
        GpuThrottleReason::PeakPowerLimit => "Peak current limit (PL4)",
        GpuThrottleReason::ProcessorHot => "Processor hot (PROCHOT)",
        GpuThrottleReason::RunningAverageThermalLimit => "Running average thermal limit",
        GpuThrottleReason::Thermal => "Thermal limit",
        GpuThrottleReason::VoltageRegulatorCurrent => "Voltage regulator current",
        GpuThrottleReason::VoltageRegulatorThermal => "Voltage regulator thermal alert",
    }
}
