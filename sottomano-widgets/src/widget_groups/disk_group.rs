mod disk_transfer_gauge;
mod disk_usage_gauge;
mod drive_health_indicator;

use iced::{Subscription, Task};
use sottomano_config::widget_groups::temperature_sensors::SensorConfig;
use sottomano_sources::{
    SnapshotDelivery,
    disk_storage::{
        self, disk_io_activity::DiskActivity, drive_temperature_health::DriveHealth,
        root_filesystem_usage::RootFilesystem,
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        tooltip_body::{Section, tooltip_source_delivery::Tooltip},
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

impl From<disk_storage::Error> for Section {
    fn from(error: disk_storage::Error) -> Self {
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
    usage: LatestSnapshot<RootFilesystem>,
    activity: LatestSnapshot<DiskActivity>,
    drive_health: LatestSnapshot<DriveHealth>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum DiskMessage {
    Usage(SnapshotDelivery<RootFilesystem>),
    DiskActivity(SnapshotDelivery<DiskActivity>),
    DriveHealth(SnapshotDelivery<DriveHealth>),
    Expansion(ExpansionMessage),
}

impl DiskGroup {
    pub const NAME: &str = "Disks";

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(drive_sensor: &SensorConfig) -> Subscription<DiskMessage> {
        Subscription::batch([
            snapshot_subscription::<RootFilesystem>(()).map(DiskMessage::Usage),
            snapshot_subscription::<DiskActivity>(()).map(DiskMessage::DiskActivity),
            snapshot_subscription::<DriveHealth>(drive_sensor.clone())
                .map(DiskMessage::DriveHealth),
        ])
    }

    pub fn refresh(target: DiskTarget, drive_sensor: &SensorConfig) -> Task<DiskMessage> {
        match target {
            DiskTarget::Usage => first_delivery::<RootFilesystem>(()).map(DiskMessage::Usage),
            DiskTarget::Read | DiskTarget::Write => {
                first_delivery::<DiskActivity>(()).map(DiskMessage::DiskActivity)
            }
            DiskTarget::DriveHealth => {
                first_delivery::<DriveHealth>(drive_sensor.clone()).map(DiskMessage::DriveHealth)
            }
        }
    }

    pub fn update(&mut self, msg: DiskMessage, clock: Clock) -> Task<DiskMessage> {
        match msg {
            DiskMessage::Usage(reading) => self.usage.receive(reading, clock),
            DiskMessage::DiskActivity(reading) => self.activity.receive(reading, clock),
            DiskMessage::DriveHealth(reading) => self.drive_health.receive(reading, clock),
            DiskMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    pub fn group_content(&self) -> GroupContent<'_, DiskMessage> {
        let activity = self.activity.get();
        GroupContent::new(
            Self::NAME,
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
    pub fn tooltip_for(&self, target: DiskTarget, clock: Clock) -> Option<Tooltip> {
        match target {
            DiskTarget::Usage => self.usage.tooltip(clock),
            DiskTarget::Read => match self.activity.get() {
                Some(activity) => self.activity.tooltip_with(activity.read.tooltip(clock)),
                None => self.activity.tooltip(clock),
            },
            DiskTarget::Write => self
                .activity
                .tooltip_with(self.activity.get()?.write.tooltip(clock)),
            DiskTarget::DriveHealth => self.drive_health.tooltip(clock),
        }
    }
}
