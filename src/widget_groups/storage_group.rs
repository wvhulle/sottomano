pub mod disk_usage_gauge;
pub mod memory_usage_gauge;

use iced::{Subscription, Task};

use crate::{
    data::source::{
        disk::{self, Disk},
        sysfs_sensors::{self, memory::Memory},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        last_poll::LastPoll,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupExpansion, WidgetGroup},
    },
};

#[derive(Default)]
pub struct StorageGroup {
    memory: LastPoll<Memory>,
    disk: LastPoll<Disk>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum StorageMessage {
    Memory(Result<Memory, sysfs_sensors::Error>),
    Disk(Result<Disk, disk::Error>),
    Expansion(ExpansionMessage),
}

impl StorageGroup {
    pub fn subscription() -> Subscription<StorageMessage> {
        Subscription::batch([
            polling::<Memory>(()).map(StorageMessage::Memory),
            polling::<Disk>(()).map(StorageMessage::Disk),
        ])
    }

    pub fn update(&mut self, msg: StorageMessage) -> Task<StorageMessage> {
        match msg {
            StorageMessage::Memory(reading) => self.memory.receive(reading),
            StorageMessage::Disk(reading) => self.disk.receive(reading),
            StorageMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn widget_group(&self) -> WidgetGroup<'_, StorageMessage> {
        WidgetGroup::new(&self.expansion, StorageMessage::Expansion)
            .reading(self.memory.display(), HoverTarget::Memory)
            .reading(self.disk.display(), HoverTarget::Disk)
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Memory => self.memory.tooltip(),
            HoverTarget::Disk => self.disk.tooltip(),
            _ => None,
        }
    }
}
