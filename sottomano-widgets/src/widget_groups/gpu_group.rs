use crate::widget_groups::gpu_group::gpu_load_gauge::GPU_SNAPSHOT_FALLBACK_GLYPH;
mod gpu_clock_label;
mod gpu_load_gauge;
mod gpu_memory_gauge;
mod gpu_power_label;
mod gpu_temperature_label;

use std::collections::BTreeMap;

use gpu_clock_label::GpuClockLabel;
use gpu_load_gauge::{gpu_load_hover_card, gpu_load_reading_row};
use gpu_memory_gauge::gpu_memory_reading_row;
use gpu_power_label::{gpu_power_hover_card, gpu_power_reading_row};
use gpu_temperature_label::GpuTemperatureLabel;
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::pci_gpu_readings::{
        GpuClocks, GpuDiscovery, GpuHandle, GpuMemory, GpuModel, GpuPowerDraw, GpuProcesses,
        GpuRuntimePower, GpuSnapshot, PciBusId,
    },
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        offline_internet_reading::{PlaceholderCause, SnapshotPlaceholder, SnapshotTrouble},
    },
    hover_cards::hover_card_body::Section,
    widget_groups::{
        gpu_group::{
            gpu_clock_label::gpu_clocks_sections,
            gpu_memory_gauge::{gpu_memory_sections, gpu_processes_sections},
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
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
        handle_failure: SnapshotPlaceholder<GpuSnapshot>,
    },
}

struct GpuDevice {
    model: GpuModel,
    access: GpuDeviceAccess,
    runtime_power: GpuRuntimePower,
    expansion: WidgetGroupExpansion,
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

fn gpu_group_identity(bus: &PciBusId) -> WidgetGroupIdentity {
    WidgetGroupIdentity::durable(WidgetGroupFamily::Gpu, bus.0.clone())
}

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
    Expansion(PciBusId, WidgetGroupExpansionMessage),
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
                .map(|(bus, reading)| GpuMessage::RuntimePower(bus, reading))
        });
        let snapshots = self
            .devices
            .iter()
            .filter(|(_, device)| device.runtime_power == GpuRuntimePower::Awake)
            .filter_map(|(bus, device)| {
                Some(
                    snapshot_subscription::<GpuSnapshot>(device.readable_handle()?.clone())
                        .with(bus.clone())
                        .map(|(bus, reading)| GpuMessage::Snapshot(bus, reading)),
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
            GpuMessage::Discovered(reading) => {
                self.discovery.receive(reading, clock);
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
                                handle_failure: SnapshotPlaceholder::caused_by(
                                    PlaceholderCause::Troubled(SnapshotTrouble::FetchFailed(
                                        failure.clone(),
                                    )),
                                ),
                            },
                        };
                        let device = GpuDevice {
                            model: gpu.model.clone(),
                            access,
                            runtime_power: GpuRuntimePower::Unreported,
                            expansion: WidgetGroupExpansion::default(),
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
            GpuMessage::Snapshot(bus, reading) => {
                if let Some(GpuDevice {
                    access: GpuDeviceAccess::Readable { snapshot, .. },
                    ..
                }) = self.devices.get_mut(&bus)
                {
                    snapshot.receive(reading, clock);
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

    fn device_items<'a>(
        &'a self,
        bus: &'a PciBusId,
        device: &'a GpuDevice,
        clock: Clock,
    ) -> [Option<GroupItem<'a, GpuInteraction>>; 5] {
        let refresh = || GpuInteraction::Refresh(bus.clone());
        let model_sections = move || {
            self.discovery
                .sections_described_by(|_| vec![Section::from(&device.model)])
        };
        let readable_snapshot = match &device.access {
            GpuDeviceAccess::Readable { snapshot, .. } => Some(snapshot),
            GpuDeviceAccess::Unreadable { .. } => None,
        };
        [
            match &device.access {
                GpuDeviceAccess::Readable { snapshot, .. } => snapshot
                    .group_item(GPU_SNAPSHOT_FALLBACK_GLYPH, gpu_load_reading_row, clock)
                    .map(|load_item| {
                        load_item
                            .with_hover_card(Box::new(move |_| {
                                snapshot
                                    .sections_described_by(gpu_load_hover_card)
                                    .into_iter()
                                    .chain(model_sections())
                                    .collect()
                            }))
                            .refreshed_by(refresh())
                    }),
                GpuDeviceAccess::Unreadable { handle_failure } => Some(
                    GroupItem::reading(handle_failure.reading_row(GPU_SNAPSHOT_FALLBACK_GLYPH))
                        .with_hover_card(Box::new(move |_| {
                            [handle_failure.source_state()]
                                .into_iter()
                                .chain(model_sections())
                                .collect()
                        }))
                        .refreshed_by(refresh()),
                ),
            },
            readable_snapshot
                .zip(device.clock_label())
                .map(|(snapshot, label)| {
                    GroupItem::reading(label.reading_row())
                        .with_hover_card(Box::new(move |_| {
                            let mut sections = snapshot.hover_card_with(label.hover_card());
                            sections.extend(self.hovered_sections(bus, |group| {
                                group.clocks.sections_described_by(gpu_clocks_sections)
                            }));
                            sections
                        }))
                        .fetching_on_hover(
                            GpuInteraction::StartClocksFetch(bus.clone()),
                            GpuInteraction::StopHoverFetches,
                        )
                        .refreshed_by(refresh())
                }),
            readable_snapshot
                .zip(device.memory())
                .map(|(snapshot, memory)| {
                    let card_memory = memory.clone();
                    GroupItem::reading(gpu_memory_reading_row(memory))
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
                        .refreshed_by(refresh())
                }),
            readable_snapshot
                .zip(device.temperature_label())
                .map(|(snapshot, label)| {
                    GroupItem::reading(label.reading_row())
                        .with_hover_card(Box::new(move |_| {
                            snapshot.hover_card_with(label.hover_card())
                        }))
                        .refreshed_by(refresh())
                }),
            readable_snapshot
                .zip(device.power_label())
                .map(|(snapshot, label)| {
                    GroupItem::reading(gpu_power_reading_row(label))
                        .with_hover_card(Box::new(move |_| {
                            snapshot.hover_card_with(gpu_power_hover_card(&label))
                        }))
                        .refreshed_by(refresh())
                }),
        ]
    }

    pub fn group_contents(
        &self,
        clock: Clock,
    ) -> impl Iterator<Item = WidgetGroupContent<'_, GpuInteraction>> {
        self.devices
            .iter()
            .zip(WidgetGroupTitle::numbered_namesakes(
                self.devices
                    .values()
                    .map(|device| device.model.names.preferred_name().to_owned()),
            ))
            .filter(|((_, device), _)| device.runtime_power == GpuRuntimePower::Awake)
            .map(move |((bus, device), group_title)| {
                let expanded_bus = bus.clone();
                WidgetGroupContent::expandable_instance(
                    gpu_group_identity(bus),
                    group_title,
                    &device.expansion,
                    move |expansion| GpuInteraction::Expansion(expanded_bus.clone(), expansion),
                    self.device_items(bus, device, clock).into_iter().flatten(),
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
            .map(move |reading| GpuMessage::Snapshot(bus.clone(), reading))
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_readings::GpuNames;

    use super::{PciBusId, WidgetGroupTitle, gpu_group_identity};

    fn titles_of(product_names: &[&str]) -> Vec<String> {
        let discovered_names: Vec<GpuNames> = product_names
            .iter()
            .map(|product_name| GpuNames::Product((*product_name).to_owned()))
            .collect();
        WidgetGroupTitle::numbered_namesakes(
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
        let on_bus = |bus: &str| gpu_group_identity(&PciBusId(bus.to_owned()));
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
