mod disk_transfer_gauge;
mod drive_health_indicator;

use std::{cmp::Ordering, collections::BTreeMap, ptr};

use iced::{Subscription, Task};
use sottomano_config::widget_groups::temperature_sensors::SensorConfig;
use sottomano_sources::{
    SnapshotDelivery,
    disk_storage::{
        block_filesystem_mounts::{BlockFilesystemDiscovery, KernelDevice},
        disk_io_activity::{DiskActivity, DiskTransfer},
        drive_temperature_health::{DriveHealth, DriveHealthInputs},
    },
};
use uom::si::f64::InformationRate;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widgets::{
        bar_widget::BarWidget,
        group_item::GroupItem,
        latest_snapshot::LatestSnapshot,
        named_reading::NamedReading,
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveTarget {
    MountDiscovery,
    Read(String),
    Write(String),
    DriveHealth(String),
}

impl From<DriveTarget> for HoverTarget {
    fn from(target: DriveTarget) -> Self {
        HoverTarget::Drive(target)
    }
}

#[derive(Default)]
struct PhysicalDiskReadings {
    activity: LatestSnapshot<DiskActivity>,
    drive_health: LatestSnapshot<DriveHealth>,
}

impl PhysicalDiskReadings {
    fn hotter_first(&self, other: &Self) -> Ordering {
        let severity = |readings: &Self| {
            readings
                .drive_health
                .get()
                .map(DriveHealth::temperature_level)
        };
        let temperature = |readings: &Self| {
            readings
                .drive_health
                .get()
                .and_then(|health| health.temperature.current())
        };
        severity(other).cmp(&severity(self)).then_with(|| {
            temperature(other)
                .partial_cmp(&temperature(self))
                .unwrap_or(Ordering::Equal)
        })
    }

    fn transfers(&self) -> impl Iterator<Item = &DiskTransfer> {
        self.activity
            .get()
            .into_iter()
            .flat_map(|activity| [&activity.read, &activity.write])
    }
}

#[derive(Default)]
pub struct PhysicalDriveGroup {
    discovery: LatestSnapshot<BlockFilesystemDiscovery>,
    disks: BTreeMap<KernelDevice, PhysicalDiskReadings>,
    expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum DriveMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    DiskActivity(KernelDevice, SnapshotDelivery<DiskActivity>),
    DriveHealth(KernelDevice, SnapshotDelivery<DriveHealth>),
    Expansion(ExpansionMessage),
}

impl PhysicalDriveGroup {
    pub const NAME: &str = "Drives";

    pub fn collapse(&mut self) {
        self.expansion.collapse();
    }

    pub fn subscription(&self, drive_sensor: &SensorConfig) -> Subscription<DriveMessage> {
        let per_disk = self.disks.keys().flat_map(|disk| {
            [
                snapshot_subscription::<DiskActivity>(disk.clone())
                    .with(disk.clone())
                    .map(|(disk, reading)| DriveMessage::DiskActivity(disk, reading)),
                snapshot_subscription::<DriveHealth>(DriveHealthInputs {
                    disk: disk.clone(),
                    sensor: drive_sensor.clone(),
                })
                .with(disk.clone())
                .map(|(disk, reading)| DriveMessage::DriveHealth(disk, reading)),
            ]
        });
        Subscription::batch(
            [snapshot_subscription::<BlockFilesystemDiscovery>(()).map(DriveMessage::Discovered)]
                .into_iter()
                .chain(per_disk),
        )
    }

    pub fn refresh(&self, target: &DriveTarget, drive_sensor: &SensorConfig) -> Task<DriveMessage> {
        let disk_named = |name: &String| self.disks.keys().find(|disk| &disk.name == name).cloned();
        let discover =
            || first_delivery::<BlockFilesystemDiscovery>(()).map(DriveMessage::Discovered);
        match target {
            DriveTarget::MountDiscovery => discover(),
            DriveTarget::Read(name) | DriveTarget::Write(name) => {
                let Some(disk) = disk_named(name) else {
                    return discover();
                };
                first_delivery::<DiskActivity>(disk.clone())
                    .map(move |reading| DriveMessage::DiskActivity(disk.clone(), reading))
            }
            DriveTarget::DriveHealth(name) => {
                let Some(disk) = disk_named(name) else {
                    return discover();
                };
                first_delivery::<DriveHealth>(DriveHealthInputs {
                    disk: disk.clone(),
                    sensor: drive_sensor.clone(),
                })
                .map(move |reading| DriveMessage::DriveHealth(disk.clone(), reading))
            }
        }
    }

    pub fn update(&mut self, msg: DriveMessage, clock: Clock) -> Task<DriveMessage> {
        match msg {
            DriveMessage::Discovered(reading) => {
                self.discovery.receive(reading, clock);
                if let Some(discovery) = self.discovery.get() {
                    self.disks = discovery
                        .whole_disks()
                        .into_iter()
                        .map(|disk| {
                            let readings = self.disks.remove(&disk).unwrap_or_default();
                            (disk, readings)
                        })
                        .collect();
                }
            }
            DriveMessage::DiskActivity(disk, reading) => {
                if let Some(readings) = self.disks.get_mut(&disk) {
                    readings.activity.receive(reading, clock);
                }
            }
            DriveMessage::DriveHealth(disk, reading) => {
                if let Some(readings) = self.disks.get_mut(&disk) {
                    readings.drive_health.receive(reading, clock);
                }
            }
            DriveMessage::Expansion(m) => {
                self.expansion.update(m);
            }
        }
        Task::none()
    }

    fn busiest_transfer(&self) -> Option<&DiskTransfer> {
        self.disks
            .values()
            .flat_map(PhysicalDiskReadings::transfers)
            .max_by(|first, second| {
                first
                    .rate
                    .partial_cmp(&second.rate)
                    .unwrap_or(Ordering::Equal)
            })
            .filter(|transfer| transfer.rate > InformationRate::default())
    }

    fn transfer_item<'a>(
        transfer: &'a DiskTransfer,
        target: DriveTarget,
        busiest: Option<&DiskTransfer>,
    ) -> GroupItem<'a, DriveMessage> {
        let is_busiest = busiest.is_some_and(|busiest| ptr::eq(busiest, transfer));
        GroupItem::reading(transfer, target).previewed_on_handle_if(is_busiest)
    }

    pub fn group_content(&self) -> GroupContent<'_, DriveMessage> {
        let names_disks = self.disks.len() > 1;
        let busiest = self.busiest_transfer();
        let mut hottest_first: Vec<(&KernelDevice, &PhysicalDiskReadings)> =
            self.disks.iter().collect();
        hottest_first.sort_by(|(_, first), (_, second)| first.hotter_first(second));
        let disk_items = hottest_first.into_iter().flat_map(move |(disk, readings)| {
            let health_target = DriveTarget::DriveHealth(disk.name.clone());
            let health_item = match readings.drive_health.display() {
                Some(health) if names_disks => Some(GroupItem::reading(
                    NamedReading {
                        reading: health,
                        shown_name: &disk.name,
                    },
                    health_target,
                )),
                _ => readings.drive_health.group_item(health_target),
            };
            let activity = readings.activity.get();
            [
                health_item,
                readings
                    .activity
                    .group_item(DriveTarget::Read(disk.name.clone())),
                activity.map(|activity| {
                    Self::transfer_item(
                        &activity.read,
                        DriveTarget::Read(disk.name.clone()),
                        busiest,
                    )
                }),
                activity.map(|activity| {
                    Self::transfer_item(
                        &activity.write,
                        DriveTarget::Write(disk.name.clone()),
                        busiest,
                    )
                }),
            ]
            .into_iter()
            .flatten()
        });
        GroupContent::new(
            Self::NAME,
            &self.expansion,
            DriveMessage::Expansion,
            self.discovery
                .group_item(DriveTarget::MountDiscovery)
                .into_iter()
                .chain(disk_items),
        )
    }

    #[must_use]
    pub fn tooltip_for(&self, target: &DriveTarget, clock: Clock) -> Option<Tooltip> {
        let readings_of = |name: &String| {
            self.disks
                .iter()
                .find(|(disk, _)| &disk.name == name)
                .map(|(_, readings)| readings)
        };
        match target {
            DriveTarget::MountDiscovery => self.discovery.tooltip(clock),
            DriveTarget::Read(name) => {
                let readings = readings_of(name)?;
                match readings.activity.get() {
                    Some(activity) => readings.activity.tooltip_with(activity.read.tooltip(clock)),
                    None => readings.activity.tooltip(clock),
                }
            }
            DriveTarget::Write(name) => {
                let readings = readings_of(name)?;
                readings
                    .activity
                    .tooltip_with(readings.activity.get()?.write.tooltip(clock))
            }
            DriveTarget::DriveHealth(name) => readings_of(name)?.drive_health.tooltip(clock),
        }
    }
}
