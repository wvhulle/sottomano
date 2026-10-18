mod disk_transfer_gauge;
mod disk_usage_gauge;
mod drive_health_indicator;

use iced::{Subscription, Task};
use vetrata_config::widget_groups::temperature_sensors::SensorConfig;
use vetrata_sources::{
    SourceDelivery,
    disk::{self, Disk, activity::DiskActivity, health::DriveHealth},
};

use crate::{
    background_work::{first_delivery, source_subscription},
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
        tooltip_body::{Section, Tooltip},
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

impl From<disk::Error> for Section {
    fn from(error: disk::Error) -> Self {
        Section::error(error)
    }
}

impl From<DiskTarget> for HoverTarget {
    fn from(target: DiskTarget) -> Self {
        HoverTarget::Disk(target)
    }
}

#[derive(Default)]
pub struct DiskGroup {
    usage: ContinuousReading<Disk>,
    activity: ContinuousReading<DiskActivity>,
    drive_health: ContinuousReading<DriveHealth>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum DiskMessage {
    Usage(SourceDelivery<Disk>),
    DiskActivity(SourceDelivery<DiskActivity>),
    DriveHealth(SourceDelivery<DriveHealth>),
    Expansion(ExpansionMessage),
}

impl DiskGroup {
    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(drive_sensor: &SensorConfig) -> Subscription<DiskMessage> {
        Subscription::batch([
            source_subscription::<Disk>(()).map(DiskMessage::Usage),
            source_subscription::<DiskActivity>(()).map(DiskMessage::DiskActivity),
            source_subscription::<DriveHealth>(drive_sensor.clone()).map(DiskMessage::DriveHealth),
        ])
    }

    pub fn refresh(target: DiskTarget, drive_sensor: &SensorConfig) -> Task<DiskMessage> {
        match target {
            DiskTarget::Usage => first_delivery::<Disk>(()).map(DiskMessage::Usage),
            DiskTarget::Read | DiskTarget::Write => {
                first_delivery::<DiskActivity>(()).map(DiskMessage::DiskActivity)
            }
            DiskTarget::DriveHealth => {
                first_delivery::<DriveHealth>(drive_sensor.clone()).map(DiskMessage::DriveHealth)
            }
        }
    }

    pub fn update(&mut self, msg: DiskMessage) -> Task<DiskMessage> {
        match msg {
            DiskMessage::Usage(reading) => self.usage.receive_delivery(reading),
            DiskMessage::DiskActivity(reading) => self.activity.receive_delivery(reading),
            DiskMessage::DriveHealth(reading) => self.drive_health.receive_delivery(reading),
            DiskMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, DiskMessage> {
        let activity = self.activity.get();
        GroupContent::new(
            &self.expansion,
            DiskMessage::Expansion,
            [
                self.usage.group_item(DiskTarget::Usage),
                self.activity.group_item(DiskTarget::Read),
                activity.map(|activity| GroupItem::reading(&activity.read, DiskTarget::Read)),
                activity.map(|activity| GroupItem::reading(&activity.write, DiskTarget::Write)),
                self.drive_health.group_item(DiskTarget::DriveHealth),
            ]
            .into_iter()
            .flatten(),
        )
    }

    #[must_use]
    pub fn tooltip_for(&self, target: DiskTarget) -> Option<Tooltip> {
        match target {
            DiskTarget::Usage => self.usage.tooltip(),
            DiskTarget::Read => match self.activity.get() {
                Some(activity) => self.activity.tooltip_with(activity.read.tooltip()),
                None => self.activity.tooltip(),
            },
            DiskTarget::Write => self
                .activity
                .tooltip_with(self.activity.get()?.write.tooltip()),
            DiskTarget::DriveHealth => self.drive_health.tooltip(),
        }
    }
}
