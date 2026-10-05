pub mod disk_transfer_gauge;
pub mod disk_usage_gauge;
pub mod drive_health_indicator;
pub mod memory_usage_gauge;

use iced::{Subscription, Task};

use crate::{
    data::source::{
        disk::{
            self, Disk,
            activity::{DiskActivity, DiskTransfer, Reading, Writing},
            health::DriveHealth,
        },
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
    disk_read: LastPoll<DiskTransfer<Reading>>,
    disk_write: LastPoll<DiskTransfer<Writing>>,
    drive_health: LastPoll<DriveHealth>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum StorageMessage {
    Memory(Result<Memory, sysfs_sensors::Error>),
    Disk(Result<Disk, disk::Error>),
    DiskActivity(Result<DiskActivity, disk::Error>),
    DriveHealth(Result<DriveHealth, disk::Error>),
    Expansion(ExpansionMessage),
}

impl StorageGroup {
    pub fn subscription() -> Subscription<StorageMessage> {
        Subscription::batch([
            polling::<Memory>(()).map(StorageMessage::Memory),
            polling::<Disk>(()).map(StorageMessage::Disk),
            polling::<DiskActivity>(()).map(StorageMessage::DiskActivity),
            polling::<DriveHealth>(()).map(StorageMessage::DriveHealth),
        ])
    }

    pub fn update(&mut self, msg: StorageMessage) -> Task<StorageMessage> {
        match msg {
            StorageMessage::Memory(reading) => self.memory.receive(reading),
            StorageMessage::Disk(reading) => self.disk.receive(reading),
            StorageMessage::DiskActivity(reading) => self.receive_disk_activity(reading),
            StorageMessage::DriveHealth(reading) => self.drive_health.receive(reading),
            StorageMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    fn receive_disk_activity(&mut self, reading: Result<DiskActivity, disk::Error>) {
        self.disk_read
            .receive(reading.clone().map(|activity| activity.read));
        self.disk_write
            .receive(reading.map(|activity| activity.write));
    }

    pub fn widget_group(&self) -> WidgetGroup<'_, StorageMessage> {
        WidgetGroup::new(&self.expansion, StorageMessage::Expansion)
            .reading(self.memory.display(), HoverTarget::Memory)
            .reading(self.disk.display(), HoverTarget::Disk)
            .reading(self.disk_read.display(), HoverTarget::DiskRead)
            .reading(self.disk_write.display(), HoverTarget::DiskWrite)
            .reading(self.drive_health.display(), HoverTarget::DriveHealth)
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Memory => self.memory.tooltip(),
            HoverTarget::Disk => self.disk.tooltip(),
            HoverTarget::DiskRead => self.disk_read.live_tooltip(),
            HoverTarget::DiskWrite => self.disk_write.live_tooltip(),
            HoverTarget::DriveHealth => self.drive_health.tooltip(),
            _ => None,
        }
    }
}
