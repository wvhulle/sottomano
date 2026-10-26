mod gpu_clock_label;
mod gpu_load_gauge;
mod gpu_memory_gauge;
mod gpu_power_label;
mod gpu_temperature_label;

use std::collections::BTreeMap;

use gpu_clock_label::GpuClockLabel;
use gpu_temperature_label::GpuTemperatureLabel;
use iced::{Subscription, Task};
use sottomano_config::Config;
use sottomano_sources::{
    SourceDelivery,
    cpu_memory_compute::pci_gpu_readings::{
        self, GpuClocks, GpuDiscovery, GpuHandle, GpuMemory, GpuModel, GpuPowerDraw, GpuProcesses,
        GpuRuntimePower, GpuSnapshot, PciBusId,
    },
};

use crate::{
    background_work::{first_delivery, source_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        tooltip_body::{ReadingSections, Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
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

impl From<pci_gpu_readings::Error> for Section {
    fn from(error: pci_gpu_readings::Error) -> Self {
        Section::error(error)
    }
}

impl From<GpuTarget> for HoverTarget {
    fn from(target: GpuTarget) -> Self {
        HoverTarget::Gpu(target)
    }
}

struct GpuDevice {
    model: GpuModel,
    handle: Option<GpuHandle>,
    snapshot: ContinuousReading<GpuSnapshot>,
    runtime_power: Option<GpuRuntimePower>,
    expansion: GroupExpansion,
}

impl GpuDevice {
    fn is_awake(&self) -> bool {
        self.runtime_power == Some(GpuRuntimePower::Awake)
    }

    fn clock_label(&self) -> Option<GpuClockLabel> {
        Some(GpuClockLabel {
            current: self.snapshot.get()?.graphics_clock?,
        })
    }

    fn memory(&self) -> Option<GpuMemory> {
        Some(self.snapshot.get()?.memory.clone())
    }

    fn temperature_label(&self) -> Option<GpuTemperatureLabel> {
        Some(GpuTemperatureLabel {
            current: self.snapshot.get()?.temperature?,
            limits: self.model.temperature_limits,
        })
    }

    fn power_label(&self) -> Option<GpuPowerDraw> {
        Some(GpuPowerDraw {
            draw: self.snapshot.get()?.power_draw?,
            limit: self.model.power_limit,
        })
    }
}

#[derive(Default)]
pub struct GpuGroup {
    discovery: ContinuousReading<GpuDiscovery>,
    devices: BTreeMap<PciBusId, GpuDevice>,
    hovered_bus: Option<PciBusId>,
    clocks: OnHoverReading<GpuClocks>,
    processes: OnHoverReading<GpuProcesses>,
}

#[derive(Debug, Clone)]
pub enum GpuMessage {
    Discovered(SourceDelivery<GpuDiscovery>),
    RuntimePower(PciBusId, SourceDelivery<GpuRuntimePower>),
    Snapshot(PciBusId, SourceDelivery<GpuSnapshot>),
    ClocksFetched(SourceDelivery<GpuClocks>),
    ProcessesFetched(SourceDelivery<GpuProcesses>),
    Expansion(PciBusId, ExpansionMessage),
}

impl GpuGroup {
    pub fn collapse(&mut self) {
        for device in self.devices.values_mut() {
            device.expansion.collapse();
        }
    }

    pub fn subscription(&self) -> Subscription<GpuMessage> {
        let runtime_powers = self.devices.keys().map(|bus| {
            source_subscription::<GpuRuntimePower>(bus.clone())
                .with(bus.clone())
                .map(|(bus, reading)| GpuMessage::RuntimePower(bus, reading))
        });
        let snapshots = self
            .devices
            .iter()
            .filter(|(_, device)| device.is_awake())
            .filter_map(|(bus, device)| {
                Some(
                    source_subscription::<GpuSnapshot>(device.handle.clone()?)
                        .with(bus.clone())
                        .map(|(bus, reading)| GpuMessage::Snapshot(bus, reading)),
                )
            });
        Subscription::batch(
            [source_subscription::<GpuDiscovery>(()).map(GpuMessage::Discovered)]
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
            .and_then(|device| device.handle.clone())
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
        sections: impl FnOnce(&Self) -> Vec<Section>,
    ) -> Vec<Section> {
        if self.hovered_bus.as_ref() == Some(bus) {
            sections(self)
        } else {
            Vec::new()
        }
    }

    pub fn update(&mut self, msg: GpuMessage, clock: Clock) -> Task<GpuMessage> {
        match msg {
            GpuMessage::Discovered(reading) => {
                self.discovery.receive_keeping_last_reading(reading, clock);
                let Some(discovery) = self.discovery.get() else {
                    return Task::none();
                };
                self.devices = discovery
                    .gpus
                    .iter()
                    .map(|gpu| {
                        let mut snapshot = ContinuousReading::default();
                        if let Err(failure) = &gpu.handle {
                            snapshot.receive_delivery(
                                SourceDelivery::Reading(Err(failure.clone())),
                                clock,
                            );
                        }
                        let device = GpuDevice {
                            model: gpu.model.clone(),
                            handle: gpu.handle.clone().ok(),
                            snapshot,
                            runtime_power: None,
                            expansion: GroupExpansion::default(),
                        };
                        (gpu.bus.clone(), device)
                    })
                    .collect();
            }
            GpuMessage::RuntimePower(bus, SourceDelivery::Reading(Ok(power))) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.runtime_power = Some(power);
                }
            }
            GpuMessage::RuntimePower(..) => {}
            GpuMessage::Snapshot(bus, reading) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.snapshot.receive_delivery(reading, clock);
                }
            }
            GpuMessage::ClocksFetched(fetched) => self.clocks.receive_delivery(fetched, clock),
            GpuMessage::ProcessesFetched(fetched) => {
                self.processes.receive_delivery(fetched, clock);
            }
            GpuMessage::Expansion(bus, m) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.expansion.update(m);
                }
            }
        }
        Task::none()
    }

    pub fn group_contents(&self) -> impl Iterator<Item = GroupContent<'_, GpuMessage>> {
        self.devices
            .iter()
            .filter(|(_, device)| device.is_awake())
            .map(|(bus, device)| {
                let expanded_bus = bus.clone();
                let target = |reading| GpuTarget {
                    bus: bus.clone(),
                    reading,
                };
                let items = [
                    device.snapshot.group_item(target(GpuReading::Load)),
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
                GroupContent::new(
                    &device.expansion,
                    move |expansion| GpuMessage::Expansion(expanded_bus.clone(), expansion),
                    items.into_iter().flatten(),
                )
            })
    }

    pub fn refresh(&self, target: &GpuTarget) -> Task<GpuMessage> {
        let Some(device) = self.devices.get(&target.bus) else {
            return first_delivery::<GpuDiscovery>(()).map(GpuMessage::Discovered);
        };
        if !device.is_awake() {
            return Task::none();
        }
        let Some(handle) = device.handle.clone() else {
            return first_delivery::<GpuDiscovery>(()).map(GpuMessage::Discovered);
        };
        let bus = target.bus.clone();
        first_delivery::<GpuSnapshot>(handle)
            .map(move |reading| GpuMessage::Snapshot(bus.clone(), reading))
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &GpuTarget, clock: Clock) -> Option<Tooltip> {
        let device = self.devices.get(&target.bus)?;
        let snapshot = &device.snapshot;
        match target.reading {
            GpuReading::Load => snapshot.tooltip_with_sections(
                self.discovery
                    .sections_described_by(|_| vec![Section::from(&device.model)]),
                clock,
            ),
            GpuReading::Clock => Some(
                snapshot
                    .tooltip_with(device.clock_label()?.tooltip(clock))?
                    .with_sections(
                        self.hovered_sections(&target.bus, |group| group.clocks.sections(clock)),
                    ),
            ),
            GpuReading::Memory => Some(
                snapshot
                    .tooltip_with(TooltipBody::sections(device.memory()?.sections(clock)))?
                    .with_sections(
                        self.hovered_sections(&target.bus, |group| group.processes.sections(clock)),
                    ),
            ),
            GpuReading::Temperature => {
                snapshot.tooltip_with(device.temperature_label()?.tooltip(clock))
            }
            GpuReading::Power => snapshot.tooltip_with(device.power_label()?.tooltip(clock)),
        }
    }
}
