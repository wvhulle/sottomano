use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_samples::{
    GpuClockSpeed, GpuClocks, GpuFanDuty, GpuPerformanceState, GpuThrottleReason, PciBusId,
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::megahertz},
};

use super::{
    GpuGroup, GpuInteraction,
    gpu_device_access::{GpuDevice, GpuDeviceAccess},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

fn megahertz_text(frequency: Frequency) -> String {
    format!("{:.0}", frequency.into_format_args(megahertz, Abbreviation))
}

#[derive(Clone, Copy)]
pub struct GpuClockLabel {
    pub current: Frequency,
}

impl GpuClockLabel {
    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "GPU clock",
            Phosphor::WaveSine(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(megahertz_text(self.current)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
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
    let mut performance_rows = Vec::new();
    if let GpuClockSpeed::Reported(ceiling) = gpu_clocks.graphics_clock_ceiling {
        performance_rows.push(LabeledValue::new(
            "Ceiling",
            Quantity::clock_megahertz(ceiling),
        ));
    }
    if let GpuClockSpeed::Reported(memory) = gpu_clocks.memory_clock {
        performance_rows.push(LabeledValue::new(
            "Memory clock",
            Quantity::clock_megahertz(memory),
        ));
    }
    if let GpuPerformanceState::PowerState(state) = gpu_clocks.performance_state {
        performance_rows.push(LabeledValue::new("Performance state", format!("P{state}")));
    }
    if let GpuFanDuty::Spinning(speed) = gpu_clocks.fan {
        performance_rows.push(LabeledValue::new("Fan", Quantity::share(speed)));
    }
    [Section::new(
        "Performance",
        dictionary_table(performance_rows),
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

impl GpuGroup {
    pub(super) fn gpu_clock_listing<'view>(
        &'view self,
        bus: &'view PciBusId,
        device: &'view GpuDevice,
    ) -> IndicatorListing<'view> {
        let GpuDeviceAccess::Readable { snapshot, .. } = &device.access else {
            return IndicatorListing::Listed(Vec::new());
        };
        IndicatorListing::Listed(
            device
                .clock_label()
                .map(|clock_label| {
                    clock_label
                        .indicator()
                        .with_hover_card(Box::new(move |_| {
                            let mut sections = snapshot.hover_card_with(clock_label.hover_card());
                            sections.extend(self.hovered_sections(bus, |group| {
                                group.clocks.sections_described_by(gpu_clocks_sections)
                            }));
                            sections
                        }))
                        .fetching_on_hover(
                            GpuInteraction::StartClocksFetch(bus.clone()),
                            GpuInteraction::StopHoverFetches,
                        )
                        .refreshed_by(GpuInteraction::Refresh(bus.clone()))
                })
                .into_iter()
                .collect(),
        )
    }
}
