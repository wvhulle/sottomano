use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_samples::{
    GpuPowerDraw, GpuPowerLimit, PciBusId,
};
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
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

fn watt_text(power: Power) -> String {
    format!("{:.0}", power.into_format_args(watt, Abbreviation))
}

pub fn gpu_power_indicator<'indicator>(power: GpuPowerDraw) -> GroupIndicator<'indicator> {
    GroupIndicator::glyph_only(
        "GPU power",
        Phosphor::Lightning(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_readout(IndicatorReadout {
        label: ReadoutLabel::Labeled(watt_text(power.draw)),
        gauge: ReadoutGauge::Ungauged,
        badge: ReadoutBadge::Unbadged,
    })
    .with_hover_card(Box::new(move |_| gpu_power_hover_card(&power)))
}

pub fn gpu_power_hover_card(power: &GpuPowerDraw) -> Vec<Section> {
    vec![Section::new(
        "Usage",
        dictionary_table(
            [
                Some(LabeledValue::new(
                    "Draw",
                    Quantity::power(power.draw).decimals(0),
                )),
                match power.limit {
                    GpuPowerLimit::Enforced(limit) => Some(LabeledValue::new(
                        "Limit",
                        Quantity::power(limit).decimals(0),
                    )),
                    GpuPowerLimit::NotExposed => None,
                },
            ]
            .into_iter()
            .flatten(),
        ),
    )]
}

impl GpuGroup {
    pub(super) fn gpu_power_listing<'view>(
        bus: &'view PciBusId,
        device: &'view GpuDevice,
    ) -> IndicatorListing<'view> {
        let GpuDeviceAccess::Readable { snapshot, .. } = &device.access else {
            return IndicatorListing::Listed(Vec::new());
        };
        IndicatorListing::Listed(
            device
                .power_label()
                .map(|power_label| {
                    gpu_power_indicator(power_label)
                        .with_hover_card(Box::new(move |_| {
                            snapshot.hover_card_with(gpu_power_hover_card(&power_label))
                        }))
                        .refreshed_by(GpuInteraction::Refresh(bus.clone()))
                })
                .into_iter()
                .collect(),
        )
    }
}
