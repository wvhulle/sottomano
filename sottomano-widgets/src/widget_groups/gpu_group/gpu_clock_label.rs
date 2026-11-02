use chrono::Locale;
use sottomano_sources::cpu_memory_compute::pci_gpu_readings::{GpuClocks, GpuThrottleReason};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Frequency, frequency::megahertz},
};

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, ReadingSections, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

fn megahertz_text(frequency: Frequency) -> String {
    format!("{:.0}", frequency.into_format_args(megahertz, Abbreviation))
}

pub struct GpuClockLabel {
    pub current: Frequency,
}

impl GroupItemReading for GpuClockLabel {
    fn name(&self) -> &'static str {
        "GPU clock"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::WaveSine(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(megahertz_text(self.current))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([LabeledValue::new(
            "Graphics clock",
            Quantity::clock_megahertz(self.current),
        )])
    }
}

impl ReadingSections for GpuClocks {
    fn sections(&self, _clock: Clock) -> Vec<Section> {
        let throttle_reasons = (!self.throttle_reasons.is_empty()).then(|| {
            Section::dictionary(
                self.throttle_reasons
                    .iter()
                    .map(|reason| LabeledValue::new(throttle_reason_text(*reason), "active")),
            )
            .titled("Throttled by")
        });
        [Section::dictionary(
            [
                self.graphics_clock_ceiling.map(|ceiling| {
                    LabeledValue::new("Ceiling", Quantity::clock_megahertz(ceiling))
                }),
                self.memory_clock.map(|memory| {
                    LabeledValue::new("Memory clock", Quantity::clock_megahertz(memory))
                }),
                self.performance_state
                    .map(|state| LabeledValue::new("Performance state", format!("P{state}"))),
                self.fan
                    .map(|speed| LabeledValue::new("Fan", Quantity::share(speed))),
            ]
            .into_iter()
            .flatten(),
        )]
        .into_iter()
        .chain(throttle_reasons)
        .collect()
    }
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
