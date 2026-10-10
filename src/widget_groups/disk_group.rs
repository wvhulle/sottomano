pub mod disk_transfer_gauge;
pub mod disk_usage_gauge;
pub mod drive_health_indicator;

use iced::{Subscription, Task};

use crate::{
    config::temperature::SensorConfig,
    data::source::disk::{
        self, Disk,
        activity::{DiskActivity, DiskTransfer, Reading, Writing},
        health::DriveHealth,
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::polling,
    widgets::{
        last_poll::LastPoll,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Default)]
pub struct DiskGroup {
    usage: LastPoll<Disk>,
    disk_read: LastPoll<DiskTransfer<Reading>>,
    disk_write: LastPoll<DiskTransfer<Writing>>,
    drive_health: LastPoll<DriveHealth>,
    pub(crate) expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum DiskMessage {
    Usage(Result<Disk, disk::Error>),
    DiskActivity(Result<DiskActivity, disk::Error>),
    DriveHealth(Result<DriveHealth, disk::Error>),
    Expansion(ExpansionMessage),
}

impl DiskGroup {
    pub fn subscription(drive_sensor: &SensorConfig) -> Subscription<DiskMessage> {
        Subscription::batch([
            polling::<Disk>(()).map(DiskMessage::Usage),
            polling::<DiskActivity>(()).map(DiskMessage::DiskActivity),
            polling::<DriveHealth>(drive_sensor.clone()).map(DiskMessage::DriveHealth),
        ])
    }

    pub fn update(&mut self, msg: DiskMessage) -> Task<DiskMessage> {
        match msg {
            DiskMessage::Usage(reading) => self.usage.receive(reading),
            DiskMessage::DiskActivity(reading) => self.receive_disk_activity(reading),
            DiskMessage::DriveHealth(reading) => self.drive_health.receive(reading),
            DiskMessage::Expansion(m) => {
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

    pub fn group_content(&self) -> GroupContent<'_, DiskMessage> {
        GroupContent::new(
            &self.expansion,
            DiskMessage::Expansion,
            [
                self.usage.group_item(HoverTarget::Disk),
                self.disk_read.group_item(HoverTarget::DiskRead),
                self.disk_write.group_item(HoverTarget::DiskWrite),
                self.drive_health.group_item(HoverTarget::DriveHealth),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn tooltip_for(&self, target: &HoverTarget) -> Option<Tooltip> {
        match target {
            HoverTarget::Disk => self.usage.tooltip(),
            HoverTarget::DiskRead => self.disk_read.live_tooltip(),
            HoverTarget::DiskWrite => self.disk_write.live_tooltip(),
            HoverTarget::DriveHealth => self.drive_health.tooltip(),
            _ => None,
        }
    }
}
