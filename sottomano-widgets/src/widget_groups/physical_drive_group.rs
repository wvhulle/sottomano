use chrono::Locale;

use crate::widgets::icon_glyph::IconGlyph;
mod bcachefs_member_row;
mod disk_space_gauge;
mod disk_transfer_gauge;
mod drive_health_indicator;

use std::collections::BTreeMap;

use bcachefs_member_row::BcachefsMemberRow;
use corredo::transitions::ExpansionState;
use disk_space_gauge::{DiskMarking, disk_space_hover_card, disk_space_reading_row};
use disk_transfer_gauge::{
    disk_activity_reading_row, disk_transfer_hover_card, disk_transfer_reading_row,
};
use drive_health_indicator::{drive_health_hover_card, drive_health_reading_row};
use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    disk_storage::{
        block_filesystem_mounts::{BlockFilesystemDiscovery, KernelDevice},
        disk_io_activity::DiskActivity,
        drive_temperature_health::{DriveHealth, DriveHealthInputs},
        whole_disk_space_occupancy::{DiskOccupancyInputs, DiskSpaceOccupancy},
    },
};
use sottomano_toml_config::widget_groups::temperature_sensors::SensorConfig;

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::group_item::GroupItem,
    group_menus::group_source_demand::GroupSourceDemand,
    hover_cards::HoverTarget,
    widget_groups::{
        storage_space_group::{
            block_filesystem_discovery_hover_card, block_filesystem_discovery_reading_row,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveTarget {
    MountDiscovery,
    Space(String),
    Read(String),
    Write(String),
    DriveHealth(String),
    BcachefsMember { disk: String, partition: String },
}

impl From<DriveTarget> for HoverTarget {
    fn from(target: DriveTarget) -> Self {
        HoverTarget::Drive(target)
    }
}

#[derive(Default)]
struct PhysicalDiskReadings {
    space: SubscribedSnapshot<DiskSpaceOccupancy>,
    activity: SubscribedSnapshot<DiskActivity>,
    drive_health: SubscribedSnapshot<DriveHealth>,
    expansion: WidgetGroupExpansion,
}

fn disk_group_title(disk: &KernelDevice) -> WidgetGroupTitle {
    WidgetGroupTitle::Named(
        format!("{} {}", WidgetGroupFamily::PhysicalDrive.title(), disk.name).into(),
    )
}

#[derive(Default)]
pub struct PhysicalDriveGroup {
    discovery: SubscribedSnapshot<BlockFilesystemDiscovery>,
    disks: BTreeMap<KernelDevice, PhysicalDiskReadings>,
    discovery_expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum DriveMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    DiskSpace(KernelDevice, Box<SnapshotDelivery<DiskSpaceOccupancy>>),
    DiskActivity(KernelDevice, SnapshotDelivery<DiskActivity>),
    DriveHealth(KernelDevice, SnapshotDelivery<DriveHealth>),
    Interaction(DriveInteraction),
}

#[derive(Debug, Clone)]
pub enum DriveInteraction {
    DiscoveryExpansion(WidgetGroupExpansionMessage),
    DiskExpansion {
        disk: KernelDevice,
        expansion: WidgetGroupExpansionMessage,
    },
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
        demand: GroupSourceDemand,
    ) -> Subscription<DriveMessage> {
        let disk_expansion = self
            .disks
            .values()
            .map(|readings| readings.expansion.state())
            .find(|expansion_state| *expansion_state == ExpansionState::Expanded)
            .unwrap_or(ExpansionState::Collapsed);
        let demand = demand.expanded_when(disk_expansion);
        let per_disk = self.disks.keys().flat_map(|disk| {
            [
                demand.while_face_shown(
                    snapshot_subscription::<DiskSpaceOccupancy>(self.occupancy_inputs(disk))
                        .with(disk.clone())
                        .map(|(disk, reading)| DriveMessage::DiskSpace(disk, Box::new(reading))),
                ),
                demand.while_rows_shown(
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
            DriveTarget::Space(name) | DriveTarget::BcachefsMember { disk: name, .. } => {
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
            DriveMessage::Interaction(DriveInteraction::DiscoveryExpansion(expansion)) => {
                self.discovery_expansion.update(expansion);
            }
            DriveMessage::Interaction(DriveInteraction::DiskExpansion { disk, expansion }) => {
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
            .map(|position| u8::try_from(position + 1).expect("fewer than 255 disks are attached"))
            .map_or(DiskMarking::OnlyDisk, |ordinal| DiskMarking::Numbered {
                ordinal,
            })
    }

    fn transfer_items<'a>(
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
    ) -> impl Iterator<Item = GroupItem<'a, DriveInteraction>> {
        readings
            .activity
            .get()
            .into_iter()
            .flat_map(move |activity| {
                [
                    GroupItem::reading(
                        disk_transfer_reading_row(&activity.read),
                        DriveTarget::Read(disk.name.clone()),
                    )
                    .with_hover_card(Box::new(move |_| {
                        readings
                            .activity
                            .hover_card_with(disk_transfer_hover_card(&activity.read))
                    })),
                    GroupItem::reading(
                        disk_transfer_reading_row(&activity.write),
                        DriveTarget::Write(disk.name.clone()),
                    )
                    .with_hover_card(Box::new(move |_| {
                        readings
                            .activity
                            .hover_card_with(disk_transfer_hover_card(&activity.write))
                    })),
                ]
            })
    }

    fn member_items<'a>(
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        locale: Locale,
    ) -> impl Iterator<Item = GroupItem<'a, DriveInteraction>> {
        readings
            .space
            .get()
            .into_iter()
            .flat_map(|occupancy| &occupancy.filesystem_shares)
            .flat_map(|share| &share.bcachefs_members)
            .map(move |device| {
                GroupItem::reading(
                    BcachefsMemberRow { device }.reading_row(locale),
                    DriveTarget::BcachefsMember {
                        disk: disk.name.clone(),
                        partition: device.partition.clone(),
                    },
                )
                .with_hover_card(Box::new(move |_| {
                    readings
                        .space
                        .hover_card_with(BcachefsMemberRow { device }.hover_card())
                }))
            })
    }

    fn disk_group<'a>(
        &'a self,
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'a, DriveInteraction> {
        let space_target = DriveTarget::Space(disk.name.clone());
        let space_item = match readings.space.get() {
            Some(occupancy) => Some(GroupItem::reading(
                disk_space_reading_row(occupancy, self.disk_marking(disk)),
                space_target,
            )),
            None => readings.space.group_item(
                IconGlyph::default(),
                |snapshot| disk_space_reading_row(snapshot, DiskMarking::OnlyDisk),
                space_target,
                clock,
            ),
        }
        .map(|space_item| {
            space_item.with_hover_card(Box::new(move |_| {
                readings.space.sections_described_by(disk_space_hover_card)
            }))
        });
        let items = space_item
            .into_iter()
            .chain(Self::member_items(disk, readings, locale))
            .chain(
                readings
                    .drive_health
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| drive_health_reading_row(snapshot),
                        DriveTarget::DriveHealth(disk.name.clone()),
                        clock,
                    )
                    .map(|health_item| {
                        health_item.with_hover_card(Box::new(move |_| {
                            readings
                                .drive_health
                                .sections_described_by(drive_health_hover_card)
                        }))
                    }),
            )
            .chain(
                readings
                    .activity
                    .group_item(
                        IconGlyph::default(),
                        |snapshot| disk_activity_reading_row(snapshot),
                        DriveTarget::Read(disk.name.clone()),
                        clock,
                    )
                    .map(|activity_item| {
                        activity_item.with_hover_card(Box::new(move |_| {
                            readings.activity.sections_described_by(|snapshot| {
                                disk_transfer_hover_card(&snapshot.read)
                            })
                        }))
                    }),
            )
            .chain(Self::transfer_items(disk, readings));
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::PhysicalDrive, disk.name.clone()),
            disk_group_title(disk),
            &readings.expansion,
            move |expansion| DriveInteraction::DiskExpansion {
                disk: disk.clone(),
                expansion,
            },
            items,
        )
    }

    #[must_use]
    pub fn group_contents(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Vec<WidgetGroupContent<'_, DriveInteraction>> {
        let discovery_group = WidgetGroupContent::expandable(
            WidgetGroupFamily::PhysicalDrive,
            &self.discovery_expansion,
            DriveInteraction::DiscoveryExpansion,
            self.discovery
                .group_item(
                    IconGlyph::default(),
                    |snapshot| block_filesystem_discovery_reading_row(snapshot),
                    DriveTarget::MountDiscovery,
                    clock,
                )
                .map(|discovery_item| {
                    discovery_item.with_hover_card(Box::new(move |_| {
                        self.discovery
                            .sections_described_by(block_filesystem_discovery_hover_card)
                    }))
                }),
        );
        [discovery_group]
            .into_iter()
            .chain(
                self.disks
                    .iter()
                    .map(|(disk, readings)| self.disk_group(disk, readings, clock, locale)),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::disk_storage::block_filesystem_mounts::KernelDevice;

    use super::{WidgetGroupFamily, disk_group_title};

    fn disk_named(name: &str) -> KernelDevice {
        KernelDevice {
            name: name.to_owned(),
        }
    }

    #[test]
    fn each_disk_gets_its_own_row_title() {
        let first_disk_title = disk_group_title(&disk_named("nvme0n1")).to_string();
        assert_eq!(
            first_disk_title,
            format!("{} nvme0n1", WidgetGroupFamily::PhysicalDrive.title())
        );
        assert_ne!(
            disk_group_title(&disk_named("nvme0n1")),
            disk_group_title(&disk_named("sda"))
        );
    }
}
