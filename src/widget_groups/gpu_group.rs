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
        group_item::GroupItem,
        hardware_section::HardwareEntries,
        last_poll::LastPoll,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

struct GpuDevice {
    model: Option<GpuModel>,
    snapshot: LastPoll<GpuSnapshot>,
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
                                snapshot: LastPoll::default(),
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
                let reading = device.snapshot.get();
                [
                    device
                        .snapshot
                        .group_item(HoverTarget::GpuLoad(bus.clone())),
                    reading.map(|reading| {
                        GroupItem::reading(&reading.vram, HoverTarget::GpuMemory(bus.clone()))
                    }),
                    device.temperature_label().map(|label| {
                        GroupItem::reading(label, HoverTarget::GpuTemperature(bus.clone()))
                    }),
                    reading.map(|reading| {
                        GroupItem::reading(reading.power, HoverTarget::GpuPower(bus.clone()))
                    }),
                ]
                .into_iter()
                .flatten()
            }),
        )
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::GpuLoad(bus) => {
                let device = self.devices.get(bus)?;
                device
                    .snapshot
                    .tooltip_with_sections(&[device.model.hardware_section()])
            }
            HoverTarget::GpuMemory(bus) => {
                let snapshot = &self.devices.get(bus)?.snapshot;
                snapshot.tooltip_with(snapshot.get()?.vram.tooltip())
            }
            HoverTarget::GpuTemperature(bus) => {
                let device = self.devices.get(bus)?;
                device
                    .snapshot
                    .tooltip_with(device.temperature_label()?.tooltip())
            }
            HoverTarget::GpuPower(bus) => {
                let snapshot = &self.devices.get(bus)?.snapshot;
                snapshot.tooltip_with(snapshot.get()?.power.tooltip())
            }
            _ => None,
        }
    }
}
