mod gpu_clock_indicator;
mod gpu_device_access;
mod gpu_device_rows;
mod gpu_load_indicator;
mod gpu_memory_indicator;
mod gpu_power_indicator;
mod gpu_temperature_indicator;

use std::collections::BTreeMap;

use gpu_device_access::{GpuDevice, GpuDeviceAccess};
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    cpu_memory_compute::pci_gpu_samples::{
        GpuClocks, GpuDiscovery, GpuProcesses, GpuRuntimePower, GpuSnapshot, PciBusId,
    },
};
use sottomano_toml_config::Config;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_indicators::{
        indicator_group_content::{IndicatorGroupExpansion, IndicatorGroupExpansionMessage},
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
        snapshot_absence::AbsenceCause,
    },
    hover_cards::hover_card_body::Section,
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

fn gpu_group_identity(bus: &PciBusId) -> IndicatorGroupIdentity {
    IndicatorGroupIdentity::durable(IndicatorGroupFamily::Gpu, bus.bus_address.clone())
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

    use super::{PciBusId, gpu_group_identity};
    use crate::group_indicators::indicator_group_identity::IndicatorGroupTitle;

    fn numbered_gpu_titles(product_names: &[&str]) -> Vec<String> {
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
        assert_eq!(
            numbered_gpu_titles(&["GeForce RTX 4090"]),
            ["GeForce RTX 4090"]
        );
    }

    #[test]
    fn different_gpus_are_not_numbered() {
        assert_eq!(
            numbered_gpu_titles(&["GeForce RTX 4090", "Arc A770"]),
            ["GeForce RTX 4090", "Arc A770"]
        );
    }

    #[test]
    fn only_later_namesakes_are_numbered() {
        assert_eq!(
            numbered_gpu_titles(&["GeForce RTX 4090", "Arc A770", "GeForce RTX 4090"]),
            ["GeForce RTX 4090", "Arc A770", "GeForce RTX 4090 2"]
        );
    }
}
