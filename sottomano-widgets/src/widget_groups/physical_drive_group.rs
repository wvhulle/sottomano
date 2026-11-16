use chrono::Locale;

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
use sottomano_toml_config::{Config, widget_groups::temperature_sensors::SensorConfig};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widget_groups::{
        group_summary::GroupSummary,
        storage_space_group::{
            block_filesystem_discovery_hover_card, block_filesystem_discovery_reading_row,
        },
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
    },
};

const HARD_DRIVES_GLYPH: Phosphor = Phosphor::HardDrives(Weight::Regular);
const DRIVE_HEALTH_GLYPH: Phosphor = Phosphor::Thermometer(Weight::Regular);
const DISK_ACTIVITY_GLYPH: Phosphor = Phosphor::TrayArrowUp(Weight::Regular);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveSource {
    MountDiscovery,
    Space(String),
    Activity(String),
    Health(String),
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
}

#[derive(Debug, Clone)]
pub enum DriveInteraction {
    DiscoveryExpansion(WidgetGroupExpansionMessage),
    DiskExpansion {
        disk: KernelDevice,
        expansion: WidgetGroupExpansionMessage,
    },
    Refresh(DriveSource),
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
                demand.while_summary_shown(
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

    fn refresh(&self, source: &DriveSource, drive_sensor: &SensorConfig) -> Task<DriveMessage> {
        let disk_named = |name: &String| self.disks.keys().find(|disk| &disk.name == name).cloned();
        let discover =
            || first_delivery::<BlockFilesystemDiscovery>(()).map(DriveMessage::Discovered);
        match source {
            DriveSource::MountDiscovery => discover(),
            DriveSource::Space(name) => {
                let Some(disk) = disk_named(name) else {
                    return discover();
                };
                first_delivery::<DiskSpaceOccupancy>(self.occupancy_inputs(&disk))
                    .map(move |reading| DriveMessage::DiskSpace(disk.clone(), Box::new(reading)))
            }
            DriveSource::Activity(name) => {
                let Some(disk) = disk_named(name) else {
                    return discover();
                };
                first_delivery::<DiskActivity>(disk.clone())
                    .map(move |reading| DriveMessage::DiskActivity(disk.clone(), reading))
            }
            DriveSource::Health(name) => {
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
        }
        Task::none()
    }

    pub fn interact(
        &mut self,
        interaction: DriveInteraction,
        config: &Config,
    ) -> Task<DriveMessage> {
        match interaction {
            DriveInteraction::DiscoveryExpansion(expansion) => {
                self.discovery_expansion.update(expansion);
            }
            DriveInteraction::DiskExpansion { disk, expansion } => {
                if let Some(readings) = self.disks.get_mut(&disk) {
                    readings.expansion.update(expansion);
                }
            }
            DriveInteraction::Refresh(source) => {
                return self.refresh(
                    &source,
                    &config.widget_groups.physical_drive.temperature_sensors,
                );
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
    ) -> impl Iterator<Item = GroupItem<'a>> {
        readings
            .activity
            .get()
            .into_iter()
            .flat_map(move |activity| {
                [
                    GroupItem::reading(disk_transfer_reading_row(&activity.read))
                        .with_hover_card(Box::new(move |_| {
                            readings
                                .activity
                                .hover_card_with(disk_transfer_hover_card(&activity.read))
                        }))
                        .refreshed_by(refresh_activity(disk)),
                    GroupItem::reading(disk_transfer_reading_row(&activity.write))
                        .with_hover_card(Box::new(move |_| {
                            readings
                                .activity
                                .hover_card_with(disk_transfer_hover_card(&activity.write))
                        }))
                        .refreshed_by(refresh_activity(disk)),
                ]
            })
    }

    fn member_items<'a>(
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        locale: Locale,
    ) -> impl Iterator<Item = GroupItem<'a>> {
        readings
            .space
            .get()
            .into_iter()
            .flat_map(|occupancy| &occupancy.filesystem_shares)
            .flat_map(|share| &share.bcachefs_members)
            .map(move |device| {
                GroupItem::reading(BcachefsMemberRow { device }.reading_row(locale))
                    .with_hover_card(Box::new(move |_| {
                        readings
                            .space
                            .hover_card_with(BcachefsMemberRow { device }.hover_card())
                    }))
                    .refreshed_by(DriveInteraction::Refresh(DriveSource::Space(
                        disk.name.clone(),
                    )))
            })
    }

    fn disk_group<'a>(
        &'a self,
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'a> {
        let group_summary = self.disk_space_summary(disk, readings, clock);
        let mut rows: Vec<_> = Self::member_items(disk, readings, locale).collect();
        Self::push_drive_health_rows(&mut rows, clock, disk, readings);
        Self::push_disk_activity_rows(&mut rows, clock, disk, readings);
        rows.extend(Self::transfer_items(disk, readings));
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::PhysicalDrive, disk.name.clone()),
            disk_group_title(disk),
            group_summary,
            &readings.expansion,
            move |expansion| DriveInteraction::DiskExpansion {
                disk: disk.clone(),
                expansion,
            },
            rows,
        )
    }

    fn disk_space_summary<'a>(
        &self,
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        clock: Clock,
    ) -> GroupSummary<'a> {
        match &readings.space {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                HARD_DRIVES_GLYPH.into(),
                Self::disk_space_item(
                    disk,
                    readings,
                    disk_space_reading_row(held_snapshot, self.disk_marking(disk)),
                ),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    HARD_DRIVES_GLYPH.into(),
                    Self::disk_space_item(
                        disk,
                        readings,
                        troubled_snapshot.shown_over(
                            disk_space_reading_row(
                                &troubled_snapshot.held_snapshot,
                                self.disk_marking(disk),
                            ),
                            clock,
                        ),
                    ),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                HARD_DRIVES_GLYPH.into(),
                Self::disk_space_item(
                    disk,
                    readings,
                    placeholder.reading_row(HARD_DRIVES_GLYPH.into()),
                ),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(HARD_DRIVES_GLYPH.into(), ReadingDisplayState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(HARD_DRIVES_GLYPH.into(), ReadingDisplayState::Off)
            }
        }
    }

    fn push_drive_health_rows<'a>(
        rows: &mut Vec<GroupItem<'a>>,
        clock: Clock,
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
    ) {
        match &readings.drive_health {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let health_row = drive_health_reading_row(held_snapshot);
                if health_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(Self::drive_health_item(disk, readings, health_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let health_row = troubled_snapshot.shown_over(
                    drive_health_reading_row(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if health_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(Self::drive_health_item(disk, readings, health_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(Self::drive_health_item(
                    disk,
                    readings,
                    placeholder.reading_row(DRIVE_HEALTH_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_disk_activity_rows<'a>(
        rows: &mut Vec<GroupItem<'a>>,
        clock: Clock,
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
    ) {
        match &readings.activity {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let activity_row = disk_activity_reading_row(held_snapshot);
                if activity_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(Self::disk_activity_item(disk, readings, activity_row));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let activity_row = troubled_snapshot.shown_over(
                    disk_activity_reading_row(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if activity_row.panel_visibility == PanelVisibility::Shown {
                    rows.push(Self::disk_activity_item(disk, readings, activity_row));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(Self::disk_activity_item(
                    disk,
                    readings,
                    placeholder.reading_row(DISK_ACTIVITY_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn disk_space_item<'a>(
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        space_row: ReadingRow<'a>,
    ) -> GroupItem<'a> {
        GroupItem::reading(space_row)
            .with_hover_card(Box::new(move |_| {
                readings.space.sections_described_by(disk_space_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::Space(
                disk.name.clone(),
            )))
    }

    fn drive_health_item<'a>(
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        health_row: ReadingRow<'a>,
    ) -> GroupItem<'a> {
        GroupItem::reading(health_row)
            .with_hover_card(Box::new(move |_| {
                readings
                    .drive_health
                    .sections_described_by(drive_health_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::Health(
                disk.name.clone(),
            )))
    }

    fn disk_activity_item<'a>(
        disk: &'a KernelDevice,
        readings: &'a PhysicalDiskReadings,
        activity_row: ReadingRow<'a>,
    ) -> GroupItem<'a> {
        GroupItem::reading(activity_row)
            .with_hover_card(Box::new(move |_| {
                readings
                    .activity
                    .sections_described_by(|snapshot| disk_transfer_hover_card(&snapshot.read))
            }))
            .refreshed_by(refresh_activity(disk))
    }

    fn mount_discovery_item<'a>(&'a self, discovery_row: ReadingRow<'a>) -> GroupItem<'a> {
        GroupItem::reading(discovery_row)
            .with_hover_card(Box::new(move |_| {
                self.discovery
                    .sections_described_by(block_filesystem_discovery_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::MountDiscovery))
    }

    #[must_use]
    pub fn group_contents(&self, clock: Clock, locale: Locale) -> Vec<WidgetGroupContent<'_>> {
        let group_summary = match &self.discovery {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => GroupSummary::summarising(
                HARD_DRIVES_GLYPH.into(),
                self.mount_discovery_item(block_filesystem_discovery_reading_row(held_snapshot)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                GroupSummary::summarising(
                    HARD_DRIVES_GLYPH.into(),
                    self.mount_discovery_item(troubled_snapshot.shown_over(
                        block_filesystem_discovery_reading_row(&troubled_snapshot.held_snapshot),
                        clock,
                    )),
                )
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => GroupSummary::summarising(
                HARD_DRIVES_GLYPH.into(),
                self.mount_discovery_item(placeholder.reading_row(HARD_DRIVES_GLYPH.into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot => {
                GroupSummary::glyph(HARD_DRIVES_GLYPH.into(), ReadingDisplayState::Pending)
            }
            SubscribedSnapshot::SourceUnavailable => {
                GroupSummary::glyph(HARD_DRIVES_GLYPH.into(), ReadingDisplayState::Off)
            }
        };
        let discovery_group = WidgetGroupContent::expandable(
            WidgetGroupFamily::PhysicalDrive,
            group_summary,
            &self.discovery_expansion,
            DriveInteraction::DiscoveryExpansion,
            [],
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

fn refresh_activity(disk: &KernelDevice) -> DriveInteraction {
    DriveInteraction::Refresh(DriveSource::Activity(disk.name.clone()))
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
