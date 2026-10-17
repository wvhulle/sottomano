mod disk_transfer_gauge;
mod disk_usage_gauge;
mod drive_health_indicator;

use iced::{Subscription, Task};

use crate::{
    config::widget_groups::temperature_sensors::SensorConfig,
    data::source::{
        SourceDelivery,
        disk::{Disk, activity::DiskActivity, health::DriveHealth},
    },
    desktop_shell::hover_tooltips::HoverTarget,
    message_subscriptions::{ForcedRefresh, source_subscription},
    widgets::{
        bar_widget::BarWidget,
        continuous_reading::ContinuousReading,
        group_item::GroupItem,
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

    pub fn subscription(
        drive_sensor: &SensorConfig,
        forced: Option<ForcedRefresh>,
    ) -> Subscription<DiskMessage> {
        Subscription::batch([
            source_subscription::<Disk>((), forced).map(DiskMessage::Usage),
            source_subscription::<DiskActivity>((), forced).map(DiskMessage::DiskActivity),
            source_subscription::<DriveHealth>(drive_sensor.clone(), forced)
                .map(DiskMessage::DriveHealth),
        ])
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
