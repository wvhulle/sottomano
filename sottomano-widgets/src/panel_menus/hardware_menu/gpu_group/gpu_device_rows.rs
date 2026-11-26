use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_samples::{GpuRuntimePower, PciBusId};

use super::{
    GpuGroup, GpuInteraction,
    gpu_device_access::{GpuDevice, GpuDeviceAccess},
    gpu_group_identity,
    gpu_load_indicator::{GPU_LOAD_GLYPH, gpu_load_indicator},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator, indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::IndicatorGroupTitle,
    },
    hover_cards::hover_card_body::Section,
};

impl GpuGroup {
    fn device_headline<'view>(
        &'view self,
        bus: &'view PciBusId,
        device: &'view GpuDevice,
    ) -> GroupIndicator<'view> {
        match &device.access {
            GpuDeviceAccess::Readable { snapshot, .. } => self
                .wired_gpu_load_indicator(
                    bus,
                    device,
                    snapshot,
                    snapshot.indicator(
                        |held_snapshot| gpu_load_indicator(held_snapshot),
                        GPU_LOAD_GLYPH.into(),
                    ),
                )
                .with_headline_glyph(Phosphor::CubeTransparent(Weight::Regular).into()),
            GpuDeviceAccess::Unreadable { handle_failure } => handle_failure
                .indicator(GPU_LOAD_GLYPH.into())
                .with_hover_card(Box::new(move |_| {
                    [handle_failure.source_state()]
                        .into_iter()
                        .chain(self.model_sections(device))
                        .collect()
                }))
                .refreshed_by(GpuInteraction::Refresh(bus.clone()))
                .with_headline_glyph(Phosphor::CubeTransparent(Weight::Regular).into()),
        }
    }

    pub(super) fn model_sections(&self, device: &GpuDevice) -> Vec<Section> {
        self.discovery
            .sections_described_by(|_| vec![Section::from(&device.model)])
    }

    pub fn group_contents(&self) -> impl Iterator<Item = IndicatorGroupContent<'_>> {
        self.devices
            .iter()
            .zip(IndicatorGroupTitle::numbered_namesakes(
                self.devices
                    .values()
                    .map(|device| device.model.names.preferred_name().to_owned()),
            ))
            .filter(|((_, device), _)| device.runtime_power == GpuRuntimePower::Awake)
            .map(move |((bus, device), group_title)| {
                let expanded_bus = bus.clone();
                let headline_indicator = self.device_headline(bus, device);
                IndicatorGroupContent::expandable_instance(
                    gpu_group_identity(bus),
                    group_title,
                    headline_indicator,
                    &device.expansion,
                    move |expansion| GpuInteraction::Expansion(expanded_bus.clone(), expansion),
                    [
                        self.gpu_load_listing(bus, device),
                        self.gpu_clock_listing(bus, device),
                        self.gpu_memory_listing(bus, device),
                        Self::gpu_temperature_listing(bus, device),
                        Self::gpu_power_listing(bus, device),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
    }
}
