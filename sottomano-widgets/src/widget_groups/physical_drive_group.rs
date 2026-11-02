mod disk_space_gauge;
mod disk_transfer_gauge;
mod drive_health_indicator;

use std::collections::BTreeMap;

use disk_space_gauge::{DiskMarking, MarkedDiskSpace};
use iced::{Subscription, Task};
use sottomano_config::widget_groups::temperature_sensors::SensorConfig;
use sottomano_sources::{
    SnapshotDelivery,
    disk_storage::{
        block_filesystem_mounts::{BlockFilesystemDiscovery, KernelDevice},
        disk_io_activity::DiskActivity,
        drive_temperature_health::{DriveHealth, DriveHealthInputs},
        whole_disk_space_occupancy::{DiskOccupancyInputs, DiskSpaceOccupancy},
    },
};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    hover_tooltips::HoverTarget,
    widget_groups::group_menus::group_source_demand::GroupSourceDemand,
    widgets::{
        group_item::GroupItem,
        group_item_reading::GroupItemReading,
        latest_snapshot::LatestSnapshot,
        tooltip_body::tooltip_source_delivery::Tooltip,
        widget_group::{ExpansionMessage, GroupContent, GroupExpansion, WidgetGroup},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveTarget {
    MountDiscovery,
    Space(String),
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
    space: LatestSnapshot<DiskSpaceOccupancy>,
    activity: LatestSnapshot<DiskActivity>,
    drive_health: LatestSnapshot<DriveHealth>,
    expansion: GroupExpansion,
}

#[derive(Default)]
pub struct PhysicalDriveGroup {
    discovery: LatestSnapshot<BlockFilesystemDiscovery>,
    disks: BTreeMap<KernelDevice, PhysicalDiskReadings>,
    discovery_expansion: GroupExpansion,
}

#[derive(Debug, Clone)]
pub enum DriveMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    DiskSpace(KernelDevice, Box<SnapshotDelivery<DiskSpaceOccupancy>>),
    DiskActivity(KernelDevice, SnapshotDelivery<DiskActivity>),
    DriveHealth(KernelDevice, SnapshotDelivery<DriveHealth>),
    DiscoveryExpansion(ExpansionMessage),
    DiskExpansion {
        disk: KernelDevice,
        expansion: ExpansionMessage,
    },
}

impl WidgetGroup for PhysicalDriveGroup {
    const TITLE: &'static str = "Drive";
}

impl PhysicalDriveGroup {
    pub fn collapse(&mut self) {
        self.discovery_expansion.collapse();
        self.disks.values_mut().for_each(|readings| {
            readings.expansion.collapse();
        });
    }

    pub fn subscription(
        &self,
        drive_sensor: &SensorConfig,
        demand: GroupSourceDemand<'_>,
    ) -> Subscription<DriveMessage> {
        let any_disk_expanded = self
            .disks
            .values()
            .any(|readings| readings.expansion.is_expanded());
        let demand = demand.expanded_when(any_disk_expanded);
        let per_disk = self.disks.keys().flat_map(|disk| {
            [
                demand.while_face_shown(
                    &[disk_space_gauge::DISK_SPACE_ITEM_NAME],
                    snapshot_subscription::<DiskSpaceOccupancy>(self.occupancy_inputs(disk))
                        .with(disk.clone())
                        .map(|(disk, reading)| DriveMessage::DiskSpace(disk, Box::new(reading))),
                ),
                demand.while_rows_shown(
                    &[
                        disk_transfer_gauge::DISK_ACTIVITY_ITEM_NAME,
                        disk_transfer_gauge::DISK_READS_ITEM_NAME,
                        disk_transfer_gauge::DISK_WRITES_ITEM_NAME,
                    ],
                    snapshot_subscription::<DiskActivity>(disk.clone())
                        .with(disk.clone())
                        .map(|(disk, reading)| DriveMessage::DiskActivity(disk, reading)),
                ),
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
            DriveTarget::Space(name) => {
                let Some(disk) = disk_named(name) else {
                    return discover();
                };
                first_delivery::<DiskSpaceOccupancy>(self.occupancy_inputs(&disk))
                    .map(move |reading| DriveMessage::DiskSpace(disk.clone(), Box::new(reading)))
            }
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
            DriveMessage::DiskSpace(disk, reading) => {
                if let Some(readings) = self.disks.get_mut(&disk) {
                    readings.space.receive(*reading, clock);
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
            DriveMessage::DiscoveryExpansion(expansion) => {
                self.discovery_expansion.update(expansion);
            }
            DriveMessage::DiskExpansion { disk, expansion } => {
                if let Some(readings) = self.disks.get_mut(&disk) {
                    readings.expansion.update(expansion);
                }
            }
        }
        Task::none()
    }

    fn occupancy_inputs(&self, disk: &KernelDevice) -> DiskOccupancyInputs {
        DiskOccupancyInputs {
            disk: disk.clone(),
            filesystems: self
                .discovery
                .get()
                .map(|discovery| discovery.filesystems.clone())
                .unwrap_or_default(),
        }
    }

    fn disk_marking(&self, disk: &KernelDevice) -> DiskMarking {
        if self.disks.len() < 2 {
            return DiskMarking::OnlyDisk;
        }
        self.disks
            .keys()
            .position(|known| known == disk)
            .and_then(|position| u8::try_from(position + 1).ok())
            .map_or(DiskMarking::OnlyDisk, |ordinal| DiskMarking::Numbered {
                ordinal,
            })
    }

    fn disk_group<'a>(
        &'a self,
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
    ) -> GroupContent<'a, DriveMessage> {
        let space_target = DriveTarget::Space(disk.name.clone());
        let space_item = match readings.space.get() {
            Some(occupancy) => Some(GroupItem::reading(
                MarkedDiskSpace {
                    occupancy,
                    marking: self.disk_marking(disk),
                },
                space_target,
            )),
            None => readings.space.group_item(space_target),
        };
        let transfer_items = readings
            .activity
            .get()
            .into_iter()
            .flat_map(move |activity| {
                [
                    GroupItem::reading(&activity.read, DriveTarget::Read(disk.name.clone())),
                    GroupItem::reading(&activity.write, DriveTarget::Write(disk.name.clone())),
                ]
            });
        let items = space_item
            .into_iter()
            .chain(
                readings
                    .drive_health
                    .group_item(DriveTarget::DriveHealth(disk.name.clone())),
            )
            .chain(
                readings
                    .activity
                    .group_item(DriveTarget::Read(disk.name.clone())),
            )
            .chain(transfer_items);
        Self::expandable_group(
            &readings.expansion,
            move |expansion| DriveMessage::DiskExpansion {
                disk: disk.clone(),
                expansion,
            },
            items,
        )
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<GroupContent<'_, DriveMessage>> {
        let discovery_group = Self::expandable_group(
            &self.discovery_expansion,
            DriveMessage::DiscoveryExpansion,
            self.discovery.group_item(DriveTarget::MountDiscovery),
        );
        [discovery_group]
            .into_iter()
            .chain(
                self.disks
                    .iter()
                    .map(|(disk, readings)| self.disk_group(disk, readings)),
            )
            .map(GroupContent::sharing_title_row)
            .collect()
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
            DriveTarget::Space(name) => readings_of(name)?.space.tooltip(clock),
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
