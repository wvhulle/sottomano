mod gpu_clock_label;
mod gpu_load_gauge;
mod gpu_memory_gauge;
mod gpu_power_label;
mod gpu_temperature_label;

use std::collections::BTreeMap;

use gpu_clock_label::GpuClockLabel;
use gpu_load_gauge::{gpu_load_hover_card, gpu_load_indicator};
use gpu_memory_gauge::gpu_memory_indicator;
use gpu_power_label::{gpu_power_hover_card, gpu_power_indicator};
use gpu_temperature_label::GpuTemperatureLabel;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::pci_gpu_samples::{
        GpuClocks, GpuDiscovery, GpuHandle, GpuMemory, GpuModel, GpuPowerDraw, GpuProcesses,
        GpuRuntimePower, GpuSnapshot, PciBusId,
    },
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{group_indicator::GroupIndicator, snapshot_absence::AbsenceCause},
    hover_cards::hover_card_body::Section,
    indicator_groups::{
        gpu_group::{
            gpu_clock_label::gpu_clocks_sections,
            gpu_memory_gauge::{gpu_memory_sections, gpu_processes_sections},
        },
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::{
            IndicatorGroupFamily, IndicatorGroupIdentity, IndicatorGroupTitle,
        },
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot, subscribed_snapshot::SubscribedSnapshot,
    },
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
enum HoveredGpu {
    #[default]
    NothingHovered,
    Hovered(PciBusId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GpuFetchedDetail {
    Clocks,
    Processes,
}

enum GpuDeviceAccess {
    Readable {
        handle: GpuHandle,
        snapshot: SubscribedSnapshot<GpuSnapshot>,
    },
    Unreadable {
        handle_failure: AbsenceCause<GpuSnapshot>,
    },
}

struct GpuDevice {
    model: GpuModel,
    access: GpuDeviceAccess,
    runtime_power: GpuRuntimePower,
    expansion: IndicatorGroupExpansion,
}

impl GpuDevice {
    fn readable_handle(&self) -> Option<&GpuHandle> {
        match &self.access {
            GpuDeviceAccess::Readable { handle, .. } => Some(handle),
            GpuDeviceAccess::Unreadable { .. } => None,
        }
    }

    fn latest_snapshot(&self) -> Option<&GpuSnapshot> {
        match &self.access {
            GpuDeviceAccess::Readable { snapshot, .. } => snapshot.get(),
            GpuDeviceAccess::Unreadable { .. } => None,
        }
    }

    fn clock_label(&self) -> Option<GpuClockLabel> {
        Some(GpuClockLabel {
            current: self.latest_snapshot()?.graphics_clock?,
        })
    }

    fn memory(&self) -> Option<GpuMemory> {
        Some(self.latest_snapshot()?.memory.clone())
    }

    fn temperature_label(&self) -> Option<GpuTemperatureLabel> {
        Some(GpuTemperatureLabel {
            current: self.latest_snapshot()?.temperature?,
            limits: self.model.temperature_limits,
        })
    }

    fn power_label(&self) -> Option<GpuPowerDraw> {
        Some(GpuPowerDraw {
            draw: self.latest_snapshot()?.power_draw?,
            limit: self.model.power_limit,
        })
    }
}

fn gpu_group_identity(bus: &PciBusId) -> IndicatorGroupIdentity {
    IndicatorGroupIdentity::durable(IndicatorGroupFamily::Gpu, bus.bus_address.clone())
}

const GPU_LOAD_GLYPH: Phosphor = Phosphor::GraphicsCard(Weight::Regular);

#[derive(Default)]
pub struct GpuGroup {
    discovery: SubscribedSnapshot<GpuDiscovery>,
    devices: BTreeMap<PciBusId, GpuDevice>,
    hovered_gpu: HoveredGpu,
    clocks: HoverFetchedSnapshot<GpuClocks>,
    processes: HoverFetchedSnapshot<GpuProcesses>,
}

#[derive(Debug)]
pub enum GpuMessage {
    Discovered(SnapshotDelivery<GpuDiscovery>),
    RuntimePower(PciBusId, SnapshotDelivery<GpuRuntimePower>),
    Snapshot(PciBusId, SnapshotDelivery<GpuSnapshot>),
    ClocksFetched(SnapshotDelivery<GpuClocks>),
    ProcessesFetched(SnapshotDelivery<GpuProcesses>),
}

#[derive(Debug, Clone)]
pub enum GpuInteraction {
    Expansion(PciBusId, IndicatorGroupExpansionMessage),
    StartClocksFetch(PciBusId),
    StartProcessesFetch(PciBusId),
    StopHoverFetches,
    Refresh(PciBusId),
}

impl GpuGroup {
    pub fn collapse(&mut self) {
        for device in self.devices.values_mut() {
            device.expansion.collapse();
        }
    }

    pub fn subscription(&self) -> Subscription<GpuMessage> {
        let runtime_powers = self.devices.keys().map(|bus| {
            snapshot_subscription::<GpuRuntimePower>(bus.clone())
                .with(bus.clone())
                .map(|(bus, sample)| GpuMessage::RuntimePower(bus, sample))
        });
        let snapshots = self
            .devices
            .iter()
            .filter(|(_, device)| device.runtime_power == GpuRuntimePower::Awake)
            .filter_map(|(bus, device)| {
                Some(
                    snapshot_subscription::<GpuSnapshot>(device.readable_handle()?.clone())
                        .with(bus.clone())
                        .map(|(bus, sample)| GpuMessage::Snapshot(bus, sample)),
                )
            });
        Subscription::batch(
            [snapshot_subscription::<GpuDiscovery>(()).map(GpuMessage::Discovered)]
                .into_iter()
                .chain(runtime_powers)
                .chain(snapshots),
        )
    }

    fn stop_hover_fetches(&mut self) {
        self.clocks.stop();
        self.processes.stop();
        self.hovered_gpu = HoveredGpu::NothingHovered;
    }

    fn start_detail_fetch(
        &mut self,
        bus: PciBusId,
        detail: GpuFetchedDetail,
        config: &Config,
        clock: Clock,
    ) -> Task<GpuMessage> {
        let readable_handle = self
            .devices
            .get(&bus)
            .and_then(|device| device.readable_handle().cloned());
        self.hovered_gpu = HoveredGpu::Hovered(bus);
        let Some(handle) = readable_handle else {
            return Task::none();
        };
        match detail {
            GpuFetchedDetail::Clocks => {
                self.clocks
                    .start_hover_fetch(handle, GpuMessage::ClocksFetched, config, clock)
            }
            GpuFetchedDetail::Processes => self.processes.start_hover_fetch(
                handle,
                GpuMessage::ProcessesFetched,
                config,
                clock,
            ),
        }
    }

    fn hovered_sections(
        &self,
        bus: &PciBusId,
        sections: impl FnOnce(&Self) -> Vec<Section>,
    ) -> Vec<Section> {
        if self.hovered_gpu == HoveredGpu::Hovered(bus.clone()) {
            sections(self)
        } else {
            Vec::new()
        }
    }

    pub fn update(&mut self, msg: GpuMessage, clock: Clock) -> Task<GpuMessage> {
        match msg {
            GpuMessage::Discovered(sample) => {
                self.discovery.receive(sample, clock);
                let Some(discovery) = self.discovery.get() else {
                    return Task::none();
                };
                self.devices = discovery
                    .gpus
                    .iter()
                    .map(|gpu| {
                        let access = match &gpu.handle {
                            Ok(handle) => GpuDeviceAccess::Readable {
                                handle: handle.clone(),
                                snapshot: SubscribedSnapshot::default(),
                            },
                            Err(failure) => GpuDeviceAccess::Unreadable {
                                handle_failure: AbsenceCause::FetchFailed(failure.clone()),
                            },
                        };
                        let device = GpuDevice {
                            model: gpu.model.clone(),
                            access,
                            runtime_power: GpuRuntimePower::Unreported,
                            expansion: IndicatorGroupExpansion::default(),
                        };
                        (gpu.bus.clone(), device)
                    })
                    .collect();
            }
            GpuMessage::RuntimePower(
                bus,
                SnapshotDelivery::SnapshotDelivered {
                    snapshot_fetch: Ok(power),
                    ..
                },
            ) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.runtime_power = power;
                }
            }
            GpuMessage::RuntimePower(..) => {}
            GpuMessage::Snapshot(bus, sample) => {
                if let Some(GpuDevice {
                    access: GpuDeviceAccess::Readable { snapshot, .. },
                    ..
                }) = self.devices.get_mut(&bus)
                {
                    snapshot.receive(sample, clock);
                }
            }
            GpuMessage::ClocksFetched(fetched) => self.clocks.receive(fetched, clock),
            GpuMessage::ProcessesFetched(fetched) => {
                self.processes.receive(fetched, clock);
            }
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: GpuInteraction,
        config: &Config,
        clock: Clock,
    ) -> Task<GpuMessage> {
        match interaction {
            GpuInteraction::Expansion(bus, expansion_message) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.expansion.update(expansion_message);
                }
                Task::none()
            }
            GpuInteraction::StartClocksFetch(bus) => {
                self.start_detail_fetch(bus, GpuFetchedDetail::Clocks, config, clock)
            }
            GpuInteraction::StartProcessesFetch(bus) => {
                self.start_detail_fetch(bus, GpuFetchedDetail::Processes, config, clock)
            }
            GpuInteraction::StopHoverFetches => {
                self.stop_hover_fetches();
                Task::none()
            }
            GpuInteraction::Refresh(bus) => self.refresh(bus),
        }
    }

    fn device_headline<'a>(
        &'a self,
        bus: &'a PciBusId,
        device: &'a GpuDevice,
    ) -> GroupIndicator<'a> {
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

    fn list_device<'a>(
        &'a self,
        bus: &'a PciBusId,
        device: &'a GpuDevice,
        indicators: &mut Vec<GroupIndicator<'a>>,
    ) {
        match &device.access {
            GpuDeviceAccess::Readable { snapshot, .. } => {
                if !snapshot.awaits_first_delivery() {
                    indicators.insert(
                        0,
                        self.wired_gpu_load_indicator(
                            bus,
                            device,
                            snapshot,
                            snapshot.indicator(
                                |held_snapshot| gpu_load_indicator(held_snapshot),
                                GPU_LOAD_GLYPH.into(),
                            ),
                        ),
                    );
                }
            }
            GpuDeviceAccess::Unreadable { handle_failure } => indicators.insert(
                0,
                handle_failure
                    .indicator(GPU_LOAD_GLYPH.into())
                    .with_hover_card(Box::new(move |_| {
                        [handle_failure.source_state()]
                            .into_iter()
                            .chain(self.model_sections(device))
                            .collect()
                    }))
                    .refreshed_by(GpuInteraction::Refresh(bus.clone())),
            ),
        }
    }

    fn model_sections(&self, device: &GpuDevice) -> Vec<Section> {
        self.discovery
            .sections_described_by(|_| vec![Section::from(&device.model)])
    }

    fn wired_gpu_load_indicator<'a>(
        &'a self,
        bus: &'a PciBusId,
        device: &'a GpuDevice,
        snapshot: &'a SubscribedSnapshot<GpuSnapshot>,
        load_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
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

    fn device_indicators<'a>(
        &'a self,
        bus: &'a PciBusId,
        device: &'a GpuDevice,
    ) -> Vec<GroupIndicator<'a>> {
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
                let mut lead_indicators = Vec::new();
                self.list_device(bus, device, &mut lead_indicators);
                IndicatorGroupContent::expandable_instance(
                    gpu_group_identity(bus),
                    group_title,
                    headline_indicator,
                    &device.expansion,
                    move |expansion| GpuInteraction::Expansion(expanded_bus.clone(), expansion),
                    lead_indicators
                        .into_iter()
                        .chain(self.device_indicators(bus, device)),
                )
            })
    }

    fn refresh(&self, bus: PciBusId) -> Task<GpuMessage> {
        let Some(device) = self.devices.get(&bus) else {
            return first_delivery::<GpuDiscovery>(()).map(GpuMessage::Discovered);
        };
        if device.runtime_power != GpuRuntimePower::Awake {
            return Task::none();
        }
        let Some(handle) = device.readable_handle().cloned() else {
            return first_delivery::<GpuDiscovery>(()).map(GpuMessage::Discovered);
        };
        first_delivery::<GpuSnapshot>(handle)
            .map(move |sample| GpuMessage::Snapshot(bus.clone(), sample))
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_samples::GpuNames;

    use super::{IndicatorGroupTitle, PciBusId, gpu_group_identity};

    fn titles_of(product_names: &[&str]) -> Vec<String> {
        let discovered_names: Vec<GpuNames> = product_names
            .iter()
            .map(|product_name| GpuNames::Product((*product_name).to_owned()))
            .collect();
        IndicatorGroupTitle::numbered_namesakes(
            discovered_names
                .iter()
                .map(|names| names.preferred_name().to_owned()),
        )
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    #[test]
    fn each_gpu_has_its_own_identity() {
        let on_bus = |bus: &str| {
            gpu_group_identity(&PciBusId {
                bus_address: bus.to_owned(),
            })
        };
        assert_ne!(on_bus("0000:03:00.0"), on_bus("0000:0b:00.0"));
    }

    #[test]
    fn a_gpu_is_titled_by_its_name() {
        assert_eq!(titles_of(&["GeForce RTX 4090"]), ["GeForce RTX 4090"]);
    }

    #[test]
    fn different_gpus_are_not_numbered() {
        assert_eq!(
            titles_of(&["GeForce RTX 4090", "Arc A770"]),
            ["GeForce RTX 4090", "Arc A770"]
        );
    }

    #[test]
    fn only_later_namesakes_are_numbered() {
        assert_eq!(
            titles_of(&["GeForce RTX 4090", "Arc A770", "GeForce RTX 4090"]),
            ["GeForce RTX 4090", "Arc A770", "GeForce RTX 4090 2"]
        );
    }
}
