mod gpu_load_gauge;
mod gpu_power_label;
mod gpu_temperature_label;
mod vram_usage_gauge;

use std::collections::BTreeMap;

use gpu_temperature_label::GpuTemperatureLabel;
use iced::{Subscription, Task};
use vetrata_config::Config;

use crate::{
    data::source::{
        SourceDelivery,
        gpu::{
            GpuClocks, GpuDiscovery, GpuModel, GpuPowerDraw, GpuProcesses, GpuSnapshot, NvmlDevice,
            PciBusId,
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{first_delivery, source_subscription},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        on_hover_reading::OnHoverReading,
        tooltip_body::{Section, Tooltip, TooltipBody},
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuReading {
    Load,
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

struct GpuDevice {
    model: Option<GpuModel>,
    nvml: Option<NvmlDevice>,
    snapshot: ContinuousReading<GpuSnapshot>,
}

impl GpuDevice {
    fn temperature_label(&self) -> Option<GpuTemperatureLabel> {
        Some(GpuTemperatureLabel {
            current: self.snapshot.get()?.temperature,
            limits: self
                .model
                .as_ref()
                .map(|model| model.temperature_limits)
                .unwrap_or_default(),
        })
    }

    fn power_label(&self) -> Option<GpuPowerDraw> {
        Some(GpuPowerDraw {
            draw: self.snapshot.get()?.power_draw,
            limit: self.model.as_ref().and_then(|model| model.power_limit),
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
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum GpuMessage {
    Discovered(SourceDelivery<GpuDiscovery>),
    Snapshot(PciBusId, SourceDelivery<GpuSnapshot>),
    ClocksFetched(SourceDelivery<GpuClocks>),
    ProcessesFetched(SourceDelivery<GpuProcesses>),
    Expansion(ExpansionMessage),
}

impl GpuGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self) -> Subscription<GpuMessage> {
        let snapshots = self.devices.iter().filter_map(|(bus, device)| {
            Some(
                source_subscription::<GpuSnapshot>(device.nvml.clone()?)
                    .with(bus.clone())
                    .map(|(bus, reading)| GpuMessage::Snapshot(bus, reading)),
            )
        });
        Subscription::batch(
            [source_subscription::<GpuDiscovery>(()).map(GpuMessage::Discovered)]
                .into_iter()
                .chain(snapshots),
        )
    }

    pub fn hover(&mut self, hovered: Option<&HoverTarget>, config: &Config) -> Task<GpuMessage> {
        self.clocks.stop();
        self.processes.stop();
        let Some(HoverTarget::Gpu(target)) = hovered else {
            self.hovered_bus = None;
            return Task::none();
        };
        self.hovered_bus = Some(target.bus.clone());
        let Some(nvml) = self
            .devices
            .get(&target.bus)
            .and_then(|device| device.nvml.clone())
        else {
            return Task::none();
        };
        Task::batch([
            self.clocks
                .start_hover_fetch(nvml.clone(), GpuMessage::ClocksFetched, config),
            self.processes
                .start_hover_fetch(nvml, GpuMessage::ProcessesFetched, config),
        ])
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

    pub fn update(&mut self, msg: GpuMessage) -> Task<GpuMessage> {
        match msg {
            GpuMessage::Discovered(reading) => {
                self.discovery.receive_keeping_last_reading(reading);
                let Some(discovery) = self.discovery.get() else {
                    return Task::none();
                };
                self.devices = discovery
                    .gpus
                    .iter()
                    .map(|gpu| {
                        let mut snapshot = ContinuousReading::default();
                        if let Err(failure) = &discovery.nvml {
                            snapshot
                                .receive_delivery(SourceDelivery::Reading(Err(failure.clone())));
                        }
                        let device = GpuDevice {
                            model: gpu.model.clone(),
                            nvml: discovery
                                .nvml
                                .as_ref()
                                .ok()
                                .map(|nvml| NvmlDevice::new(gpu.bus.clone(), nvml)),
                            snapshot,
                        };
                        (gpu.bus.clone(), device)
                    })
                    .collect();
            }
            GpuMessage::Snapshot(bus, reading) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.snapshot.receive_delivery(reading);
                }
            }
            GpuMessage::ClocksFetched(fetched) => self.clocks.receive_delivery(fetched),
            GpuMessage::ProcessesFetched(fetched) => self.processes.receive_delivery(fetched),
            GpuMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, GpuMessage> {
        GroupContent::new(
            &self.expansion,
            GpuMessage::Expansion,
            self.devices.iter().flat_map(|(bus, device)| {
                let target = |reading| GpuTarget {
                    bus: bus.clone(),
                    reading,
                };
                let snapshot = device.snapshot.get();
                [
                    device.snapshot.group_item(target(GpuReading::Load)),
                    snapshot.map(|snapshot| {
                        GroupItem::reading(snapshot.vram, target(GpuReading::Memory))
                    }),
                    device
                        .temperature_label()
                        .map(|label| GroupItem::reading(label, target(GpuReading::Temperature))),
                    device
                        .power_label()
                        .map(|label| GroupItem::reading(label, target(GpuReading::Power))),
                ]
                .into_iter()
                .flatten()
            }),
        )
    }

    pub fn refresh(&self, target: &GpuTarget) -> Task<GpuMessage> {
        let Some(nvml) = self
            .devices
            .get(&target.bus)
            .and_then(|device| device.nvml.clone())
        else {
            return first_delivery::<GpuDiscovery>(()).map(GpuMessage::Discovered);
        };
        let bus = target.bus.clone();
        first_delivery::<GpuSnapshot>(nvml)
            .map(move |reading| GpuMessage::Snapshot(bus.clone(), reading))
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &GpuTarget) -> Option<Tooltip> {
        let device = self.devices.get(&target.bus)?;
        let snapshot = &device.snapshot;
        match target.reading {
            GpuReading::Load => snapshot.tooltip_with_sections(
                [
                    self.hovered_sections(&target.bus, |group| {
                        group.clocks.sections(GpuClocks::sections)
                    }),
                    self.discovery.sections(|_| {
                        vec![Section::hardware_identity(
                            device.model.iter().flat_map(GpuModel::hardware_entries),
                        )]
                    }),
                ]
                .concat(),
            ),
            GpuReading::Memory => Some(
                snapshot
                    .tooltip_with(TooltipBody::sections(snapshot.get()?.vram.sections()))?
                    .with_sections(self.hovered_sections(&target.bus, |group| {
                        group.processes.sections(GpuProcesses::sections)
                    })),
            ),
            GpuReading::Temperature => snapshot.tooltip_with(device.temperature_label()?.tooltip()),
            GpuReading::Power => snapshot.tooltip_with(device.power_label()?.tooltip()),
        }
    }
}
