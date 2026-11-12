mod gpu_clock_label;
mod gpu_load_gauge;
mod gpu_memory_gauge;
mod gpu_power_label;
mod gpu_temperature_label;

use std::collections::BTreeMap;

use gpu_clock_label::GpuClockLabel;
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
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{
        HoverTarget,
        hover_card_body::{
            HoverCardBody, ReadingSections, Section,
            hover_card_source_delivery::{DeliveryTiming, HoverCard, SourcedSections},
        },
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        hover_fetched_snapshot::HoverFetchedSnapshot,
        latest_snapshot::{FailedSnapshotFetch, HidesFetchFailures, LatestSnapshot},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuReading {
    Load,
    Clock,
    Memory,
    Temperature,
    Power,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuTarget {
    bus: PciBusId,
    reading: GpuReading,
}

impl From<GpuTarget> for HoverTarget {
    fn from(target: GpuTarget) -> Self {
        HoverTarget::Gpu(target)
    }
}

enum GpuDeviceAccess {
    Readable {
        handle: GpuHandle,
        snapshot: LatestSnapshot<GpuSnapshot>,
    },
    Unreadable {
        handle_failure: FailedSnapshotFetch<GpuSnapshot>,
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
    discovery: LatestSnapshot<GpuDiscovery, HidesFetchFailures>,
    devices: BTreeMap<PciBusId, GpuDevice>,
    hovered_bus: Option<PciBusId>,
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
    Interaction(GpuInteraction),
}

#[derive(Debug, Clone)]
pub enum GpuInteraction {
    Expansion(PciBusId, WidgetGroupExpansionMessage),
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

    pub fn hover(
        &mut self,
        hovered: Option<&HoverTarget>,
        config: &Config,
        clock: Clock,
    ) -> Task<GpuMessage> {
        self.clocks.stop();
        self.processes.stop();
        let Some(HoverTarget::Gpu(target)) = hovered else {
            self.hovered_bus = None;
            return Task::none();
        };
        self.hovered_bus = Some(target.bus.clone());
        let Some(handle) = self
            .devices
            .get(&target.bus)
            .and_then(|device| device.readable_handle().cloned())
        else {
            return Task::none();
        };
        match target.reading {
            GpuReading::Clock => {
                self.clocks
                    .start_hover_fetch(handle, GpuMessage::ClocksFetched, config, clock)
            }
            GpuReading::Memory => self.processes.start_hover_fetch(
                handle,
                GpuMessage::ProcessesFetched,
                config,
                clock,
            ),
            GpuReading::Load | GpuReading::Temperature | GpuReading::Power => Task::none(),
        }
    }

    fn hovered_sections(
        &self,
        bus: &PciBusId,
        sections: impl FnOnce(&Self) -> Option<SourcedSections>,
    ) -> Option<SourcedSections> {
        if self.hovered_bus.as_ref() == Some(bus) {
            sections(self)
        } else {
            None
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
                                snapshot: LatestSnapshot::default(),
                            },
                            Err(failure) => GpuDeviceAccess::Unreadable {
                                handle_failure: FailedSnapshotFetch::from_fetch_error(
                                    failure.clone(),
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
            GpuMessage::Interaction(interaction) => self.interact(interaction),
        }
        Task::none()
    }

    fn interact(&mut self, interaction: GpuInteraction) {
        match interaction {
            GpuInteraction::Expansion(bus, expansion_message) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.expansion.update(expansion_message);
                }
            }
        }
    }

    pub fn group_contents(&self) -> impl Iterator<Item = WidgetGroupContent<'_, GpuInteraction>> {
        self.devices
            .iter()
            .zip(WidgetGroupTitle::numbered_namesakes(
                self.devices
                    .values()
                    .map(|device| device.model.names.preferred_name().to_owned()),
            ))
            .filter(|((_, device), _)| device.runtime_power == GpuRuntimePower::Awake)
            .map(|((bus, device), group_title)| {
                let expanded_bus = bus.clone();
                let target = |reading| GpuTarget {
                    bus: bus.clone(),
                    reading,
                };
                let items = [
                    match &device.access {
                        GpuDeviceAccess::Readable { snapshot, .. } => {
                            snapshot.group_item(target(GpuReading::Load))
                        }
                        GpuDeviceAccess::Unreadable { handle_failure } => {
                            Some(GroupItem::reading(handle_failure, target(GpuReading::Load)))
                        }
                    },
                    device
                        .clock_label()
                        .map(|label| GroupItem::reading(label, target(GpuReading::Clock))),
                    device
                        .memory()
                        .map(|memory| GroupItem::reading(memory, target(GpuReading::Memory))),
                    device
                        .temperature_label()
                        .map(|label| GroupItem::reading(label, target(GpuReading::Temperature))),
                    device
                        .power_label()
                        .map(|label| GroupItem::reading(label, target(GpuReading::Power))),
                ];
                WidgetGroupContent::expandable_instance(
                    gpu_group_identity(bus),
                    group_title,
                    &device.expansion,
                    move |expansion| GpuInteraction::Expansion(expanded_bus.clone(), expansion),
                    items.into_iter().flatten(),
                )
            })
    }

    pub fn refresh(&self, target: &GpuTarget) -> Task<GpuMessage> {
        let Some(device) = self.devices.get(&target.bus) else {
            return first_delivery::<GpuDiscovery>(()).map(GpuMessage::Discovered);
        };
        if device.runtime_power != GpuRuntimePower::Awake {
            return Task::none();
        }
        let Some(handle) = device.readable_handle().cloned() else {
            return first_delivery::<GpuDiscovery>(()).map(GpuMessage::Discovered);
        };
        let bus = target.bus.clone();
        first_delivery::<GpuSnapshot>(handle)
            .map(move |reading| GpuMessage::Snapshot(bus.clone(), reading))
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &GpuTarget, clock: Clock) -> Option<HoverCard> {
        let device = self.devices.get(&target.bus)?;
        let model_sections = self
            .discovery
            .sections_described_by(|_| vec![Section::from(&device.model)]);
        let snapshot = match &device.access {
            GpuDeviceAccess::Readable { snapshot, .. } => snapshot,
            GpuDeviceAccess::Unreadable { handle_failure } => {
                return Some(
                    HoverCard::sourced(
                        handle_failure.hover_card(clock),
                        DeliveryTiming::SingleRead,
                    )
                    .with_groups(model_sections),
                );
            }
        };
        match target.reading {
            GpuReading::Load => snapshot.hover_card_with_groups(model_sections, clock),
            GpuReading::Clock => Some(
                snapshot
                    .hover_card_with(device.clock_label()?.hover_card(clock))?
                    .with_groups(
                        self.hovered_sections(&target.bus, |group| group.clocks.sections(clock)),
                    ),
            ),
            GpuReading::Memory => Some(
                snapshot
                    .hover_card_with(HoverCardBody::sections(device.memory()?.sections(clock)))?
                    .with_groups(
                        self.hovered_sections(&target.bus, |group| group.processes.sections(clock)),
                    ),
            ),
            GpuReading::Temperature => {
                snapshot.hover_card_with(device.temperature_label()?.hover_card(clock))
            }
            GpuReading::Power => snapshot.hover_card_with(device.power_label()?.hover_card(clock)),
        }
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
