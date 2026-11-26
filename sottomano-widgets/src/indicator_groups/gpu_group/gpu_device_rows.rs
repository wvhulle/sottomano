use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_samples::{
    GpuRuntimePower, GpuSnapshot, PciBusId,
};

use super::{
    GpuGroup, GpuInteraction,
    gpu_device_access::{GpuDevice, GpuDeviceAccess},
    gpu_group_identity,
    gpu_load_gauge::{gpu_load_hover_card, gpu_load_indicator},
    gpu_memory_gauge::gpu_memory_indicator,
    gpu_power_label::{gpu_power_hover_card, gpu_power_indicator},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::GroupIndicator,
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        gpu_group::{
            gpu_clock_label::gpu_clocks_sections,
            gpu_memory_gauge::{gpu_memory_sections, gpu_processes_sections},
        },
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::IndicatorGroupTitle,
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const GPU_LOAD_GLYPH: Phosphor = Phosphor::GraphicsCard(Weight::Regular);

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

    fn device_load_listing<'view>(
        &'view self,
        bus: &'view PciBusId,
        device: &'view GpuDevice,
    ) -> IndicatorListing<'view> {
        match &device.access {
            GpuDeviceAccess::Readable { snapshot, .. } => snapshot
                .indicator_listing(
                    |held_snapshot| vec![gpu_load_indicator(held_snapshot)],
                    GPU_LOAD_GLYPH.into(),
                )
                .map_indicators(|load_row| {
                    self.wired_gpu_load_indicator(bus, device, snapshot, load_row)
                }),
            GpuDeviceAccess::Unreadable { handle_failure } => IndicatorListing::Incomplete {
                listed_indicators: Vec::new(),
                unread_sources: vec![
                    handle_failure
                        .indicator(GPU_LOAD_GLYPH.into())
                        .with_hover_card(Box::new(move |_| {
                            [handle_failure.source_state()]
                                .into_iter()
                                .chain(self.model_sections(device))
                                .collect()
                        }))
                        .refreshed_by(GpuInteraction::Refresh(bus.clone())),
                ],
            },
        }
    }

    fn model_sections(&self, device: &GpuDevice) -> Vec<Section> {
        self.discovery
            .sections_described_by(|_| vec![Section::from(&device.model)])
    }

    fn wired_gpu_load_indicator<'view>(
        &'view self,
        bus: &'view PciBusId,
        device: &'view GpuDevice,
        snapshot: &'view SubscribedSnapshot<GpuSnapshot>,
        load_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        load_indicator
            .with_hover_card(Box::new(move |_| {
                snapshot
                    .sections_described_by(gpu_load_hover_card)
                    .into_iter()
                    .chain(self.model_sections(device))
                    .collect()
            }))
            .refreshed_by(GpuInteraction::Refresh(bus.clone()))
    }

    fn device_indicators<'view>(
        &'view self,
        bus: &'view PciBusId,
        device: &'view GpuDevice,
    ) -> Vec<GroupIndicator<'view>> {
        let GpuDeviceAccess::Readable { snapshot, .. } = &device.access else {
            return Vec::new();
        };
        let refresh = || GpuInteraction::Refresh(bus.clone());
        let mut rows = Vec::new();
        if let Some(clock_label) = device.clock_label() {
            rows.push(
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
                    .refreshed_by(refresh()),
            );
        }
        if let Some(memory) = device.memory() {
            let card_memory = memory.clone();
            rows.push(
                gpu_memory_indicator(memory)
                    .with_hover_card(Box::new(move |_| {
                        let mut sections =
                            snapshot.hover_card_with(gpu_memory_sections(&card_memory));
                        sections.extend(self.hovered_sections(bus, |group| {
                            group
                                .processes
                                .sections_described_by(gpu_processes_sections)
                        }));
                        sections
                    }))
                    .fetching_on_hover(
                        GpuInteraction::StartProcessesFetch(bus.clone()),
                        GpuInteraction::StopHoverFetches,
                    )
                    .refreshed_by(refresh()),
            );
        }
        if let Some(temperature_label) = device.temperature_label() {
            rows.push(
                temperature_label
                    .indicator()
                    .with_hover_card(Box::new(move |_| {
                        snapshot.hover_card_with(temperature_label.hover_card())
                    }))
                    .refreshed_by(refresh()),
            );
        }
        if let Some(power_label) = device.power_label() {
            rows.push(
                gpu_power_indicator(power_label)
                    .with_hover_card(Box::new(move |_| {
                        snapshot.hover_card_with(gpu_power_hover_card(&power_label))
                    }))
                    .refreshed_by(refresh()),
            );
        }
        rows
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
                        self.device_load_listing(bus, device),
                        IndicatorListing::Listed(self.device_indicators(bus, device)),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
    }
}
