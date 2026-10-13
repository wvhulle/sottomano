mod disk_transfer_gauge;
mod disk_usage_gauge;
mod drive_health_indicator;

use iced::{Subscription, Task};

use crate::{
    config::widget_groups::temperature_sensors::SensorConfig,
    data::source::{
        Delivery,
        disk::{
            Disk,
            activity::{DiskActivity, DiskTransfer, Reading, Writing},
            health::DriveHealth,
        },
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, polling},
    widgets::{
        continuous_reading::ContinuousReading,
        tooltip_body::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskTarget {
    Usage,
    Read,
    Write,
    DriveHealth,
}

impl From<DiskTarget> for HoverTarget {
    fn from(target: DiskTarget) -> Self {
        HoverTarget::Disk(target)
    }
}

#[derive(Default)]
pub struct DiskGroup {
    usage: ContinuousReading<Disk>,
    disk_read: ContinuousReading<DiskTransfer<Reading>>,
    disk_write: ContinuousReading<DiskTransfer<Writing>>,
    drive_health: ContinuousReading<DriveHealth>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum DiskMessage {
    Usage(Delivery<Disk>),
    DiskActivity(Delivery<DiskActivity>),
    DriveHealth(Delivery<DriveHealth>),
    Expansion(ExpansionMessage),
}

impl DiskGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(
        drive_sensor: &SensorConfig,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<DiskMessage> {
        Subscription::batch([
            polling::<Disk>((), forced).map(DiskMessage::Usage),
            polling::<DiskActivity>((), forced).map(DiskMessage::DiskActivity),
            polling::<DriveHealth>(drive_sensor.clone(), forced).map(DiskMessage::DriveHealth),
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

    fn receive_disk_activity(&mut self, delivery: Delivery<DiskActivity>) {
        match delivery {
            Delivery::Absent => {
                self.disk_read.receive(Delivery::Absent);
                self.disk_write.receive(Delivery::Absent);
            }
            Delivery::Reading(reading) => {
                self.disk_read.receive(Delivery::Reading(
                    reading.clone().map(|activity| activity.read),
                ));
                self.disk_write
                    .receive(Delivery::Reading(reading.map(|activity| activity.write)));
            }
        }
    }

    pub fn group_content(&self) -> GroupContent<'_, DiskMessage> {
        GroupContent::new(
            &self.expansion,
            DiskMessage::Expansion,
            [
                self.usage.group_item(DiskTarget::Usage),
                self.disk_read.group_item(DiskTarget::Read),
                self.disk_write.group_item(DiskTarget::Write),
                self.drive_health.group_item(DiskTarget::DriveHealth),
            ]
            .into_iter()
            .flatten(),
        )
    }

    pub fn tooltip_for(&self, target: DiskTarget) -> Option<Tooltip> {
        match target {
            DiskTarget::Usage => self.usage.tooltip(),
            DiskTarget::Read => self.disk_read.tooltip(),
            DiskTarget::Write => self.disk_write.tooltip(),
            DiskTarget::DriveHealth => self.drive_health.tooltip(),
        }
    }
}
