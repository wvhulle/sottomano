pub mod gpu_load_gauge;
pub mod gpu_power_label;
pub mod gpu_temperature_label;
pub mod vram_usage_gauge;

use std::collections::BTreeMap;

use gpu_temperature_label::GpuTemperatureLabel;
use iced::{Subscription, Task};

use crate::{
    config::Config,
    data::source::gpu::{
        self, GpuClocks, GpuDiscovery, GpuModel, GpuPowerDraw, GpuProcesses, GpuSnapshot,
        NvmlDevice, PciBusId,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{polled, polling},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        hardware_section::HardwareEntries,
        on_hover_reading::{HoverFetched, OnHoverReading},
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
    devices: BTreeMap<PciBusId, GpuDevice>,
    hovered_bus: Option<PciBusId>,
    clocks: OnHoverReading<GpuClocks>,
    processes: OnHoverReading<GpuProcesses>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum GpuMessage {
    Discovered(GpuDiscovery),
    Snapshot(PciBusId, Result<GpuSnapshot, gpu::Error>),
    ClocksFetched(HoverFetched<GpuClocks>),
    ProcessesFetched(HoverFetched<GpuProcesses>),
    Expansion(ExpansionMessage),
}

impl GpuGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn probe() -> Task<GpuMessage> {
        Task::future(GpuDiscovery::bound_to_nvidia()).map(GpuMessage::Discovered)
    }

    pub fn subscription(&self) -> Subscription<GpuMessage> {
        Subscription::batch(self.devices.iter().filter_map(|(bus, device)| {
            Some(
                polling::<GpuSnapshot>(device.nvml.clone()?)
                    .with(bus.clone())
                    .map(|(bus, reading)| GpuMessage::Snapshot(bus, reading)),
            )
        }))
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
            self.clocks.fetch(
                polled::<GpuClocks>(nvml.clone()),
                GpuMessage::ClocksFetched,
                config,
            ),
            self.processes.fetch(
                polled::<GpuProcesses>(nvml),
                GpuMessage::ProcessesFetched,
                config,
            ),
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
            GpuMessage::Discovered(discovery) => {
                self.devices = discovery
                    .gpus
                    .into_iter()
                    .map(|gpu| {
                        let mut snapshot = ContinuousReading::default();
                        if let Err(failure) = &discovery.nvml {
                            snapshot.receive(Err::<GpuSnapshot, _>(failure.clone()));
                        }
                        let device = GpuDevice {
                            model: gpu.model,
                            nvml: discovery
                                .nvml
                                .as_ref()
                                .ok()
                                .map(|nvml| NvmlDevice::new(gpu.bus.clone(), nvml)),
                            snapshot,
                        };
                        (gpu.bus, device)
                    })
                    .collect();
            }
            GpuMessage::Snapshot(bus, reading) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.snapshot.receive(reading);
                }
            }
            GpuMessage::ClocksFetched(fetched) => self.clocks.receive(fetched),
            GpuMessage::ProcessesFetched(fetched) => self.processes.receive(fetched),
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

    pub fn tooltip_for(&self, target: &GpuTarget) -> Option<Tooltip> {
        let device = self.devices.get(&target.bus)?;
        let snapshot = &device.snapshot;
        match target.reading {
            GpuReading::Load => snapshot.tooltip_with_sections(
                &[
                    self.hovered_sections(&target.bus, |group| {
                        group.clocks.sections(GpuClocks::sections)
                    }),
                    vec![device.model.hardware_section()],
                ]
                .concat(),
            ),
            GpuReading::Memory => snapshot.tooltip_with(TooltipBody::sections(
                &[
                    snapshot.get()?.vram.sections(),
                    self.hovered_sections(&target.bus, |group| {
                        group.processes.sections(GpuProcesses::sections)
                    }),
                ]
                .concat(),
            )),
            GpuReading::Temperature => snapshot.tooltip_with(device.temperature_label()?.tooltip()),
            GpuReading::Power => snapshot.tooltip_with(device.power_label()?.tooltip()),
        }
    }
}
