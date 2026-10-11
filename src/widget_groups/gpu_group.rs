pub mod gpu_load_gauge;
pub mod gpu_power_label;
pub mod gpu_temperature_label;
pub mod vram_usage_gauge;

use std::collections::BTreeMap;

use gpu_temperature_label::GpuTemperatureLabel;
use iced::{Subscription, Task};

use crate::{
    data::source::gpu::{self, DiscoveredGpu, GpuModel, GpuSnapshot, PciBusId},
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        hardware_section::HardwareEntries,
        tooltip_body::Tooltip,
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
}

#[derive(Default)]
pub struct GpuGroup {
    devices: BTreeMap<PciBusId, GpuDevice>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum GpuMessage {
    Discovered(Vec<DiscoveredGpu>),
    Snapshot(PciBusId, Result<GpuSnapshot, gpu::Error>),
    Expansion(ExpansionMessage),
}

impl GpuGroup {
    pub fn probe() -> Task<GpuMessage> {
        Task::future(DiscoveredGpu::bound_to_nvidia()).map(GpuMessage::Discovered)
    }

    pub fn subscription(&self) -> Subscription<GpuMessage> {
        Subscription::batch(self.devices.keys().map(|bus| {
            polling::<GpuSnapshot>(bus.clone())
                .with(bus.clone())
                .map(|(bus, reading)| GpuMessage::Snapshot(bus, reading))
        }))
    }

    pub fn update(&mut self, msg: GpuMessage) -> Task<GpuMessage> {
        match msg {
            GpuMessage::Discovered(gpus) => {
                self.devices = gpus
                    .into_iter()
                    .map(|gpu| {
                        (
                            gpu.bus,
                            GpuDevice {
                                model: gpu.model,
                                snapshot: ContinuousReading::default(),
                            },
                        )
                    })
                    .collect();
            }
            GpuMessage::Snapshot(bus, reading) => {
                if let Some(device) = self.devices.get_mut(&bus) {
                    device.snapshot.receive(reading);
                }
            }
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
                        GroupItem::reading(&snapshot.vram, target(GpuReading::Memory))
                    }),
                    device
                        .temperature_label()
                        .map(|label| GroupItem::reading(label, target(GpuReading::Temperature))),
                    snapshot.map(|snapshot| {
                        GroupItem::reading(snapshot.power, target(GpuReading::Power))
                    }),
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
            GpuReading::Load => snapshot.tooltip_with_sections(&[device.model.hardware_section()]),
            GpuReading::Memory => snapshot.tooltip_with(snapshot.get()?.vram.tooltip()),
            GpuReading::Temperature => snapshot.tooltip_with(device.temperature_label()?.tooltip()),
            GpuReading::Power => snapshot.tooltip_with(snapshot.get()?.power.tooltip()),
        }
    }
}
