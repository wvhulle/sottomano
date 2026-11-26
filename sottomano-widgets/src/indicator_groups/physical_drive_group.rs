mod bcachefs_member_indicator;
mod disk_group_rows;
mod disk_space_gauge;
mod disk_transfer_gauge;
mod drive_health_indicator;

use std::collections::BTreeMap;

use chrono::Locale;
use corredo::transitions::ExpansionState;
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
use sottomano_toml_config::{Config, indicator_groups::temperature_sensors::SensorConfig};

use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::group_indicator::GroupIndicator,
    group_menus::group_source_demand::GroupSourceDemand,
    indicator_groups::{
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupTitle},
        storage_space_group::{
            block_filesystem_discovery_hover_card, block_filesystem_discovery_indicator,
        },
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const HARD_DRIVES_GLYPH: Phosphor = Phosphor::HardDrives(Weight::Regular);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveSource {
    MountDiscovery,
    Space(String),
    Activity(String),
    Health(String),
}

#[derive(Default)]
struct PhysicalDiskSamples {
    space: SubscribedSnapshot<DiskSpaceOccupancy>,
    activity: SubscribedSnapshot<DiskActivity>,
    drive_health: SubscribedSnapshot<DriveHealth>,
    expansion: IndicatorGroupExpansion,
}

fn disk_group_title(disk: &KernelDevice) -> IndicatorGroupTitle {
    IndicatorGroupTitle::Named(
        format!(
            "{} {}",
            IndicatorGroupFamily::PhysicalDrive.title(),
            disk.name
        )
        .into(),
    )
}

#[derive(Default)]
pub struct PhysicalDriveGroup {
    discovery: SubscribedSnapshot<BlockFilesystemDiscovery>,
    disks: BTreeMap<KernelDevice, PhysicalDiskSamples>,
    discovery_expansion: IndicatorGroupExpansion,
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
    DiscoveryExpansion(IndicatorGroupExpansionMessage),
    DiskExpansion {
        disk: KernelDevice,
        expansion: IndicatorGroupExpansionMessage,
    },
    Refresh(DriveSource),
}

impl PhysicalDriveGroup {
    pub fn collapse(&mut self) {
        self.discovery_expansion.collapse();
        self.disks.values_mut().for_each(|samples| {
            samples.expansion.collapse();
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
            .map(|samples| samples.expansion.state)
            .find(|expansion_state| *expansion_state == ExpansionState::Expanded)
            .unwrap_or(ExpansionState::Collapsed);
        let demand = demand.expanded_when(disk_expansion);
        let per_disk = self.disks.keys().flat_map(|disk| {
            [
                demand.while_headline_shown(
                    snapshot_subscription::<DiskSpaceOccupancy>(self.occupancy_inputs(disk))
                        .with(disk.clone())
                        .map(|(disk, sample)| DriveMessage::DiskSpace(disk, Box::new(sample))),
                ),
                demand.while_indicators_shown(
                    snapshot_subscription::<DiskActivity>(disk.clone())
                        .with(disk.clone())
                        .map(|(disk, sample)| DriveMessage::DiskActivity(disk, sample)),
                ),
                snapshot_subscription::<DriveHealth>(DriveHealthInputs {
                    disk: disk.clone(),
                    sensor: drive_sensor.clone(),
                })
                .with(disk.clone())
                .map(|(disk, sample)| DriveMessage::DriveHealth(disk, sample)),
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
                    .map(move |sample| DriveMessage::DiskSpace(disk.clone(), Box::new(sample)))
            }
            DriveSource::Activity(name) => {
                let Some(disk) = disk_named(name) else {
                    return discover();
                };
                first_delivery::<DiskActivity>(disk.clone())
                    .map(move |sample| DriveMessage::DiskActivity(disk.clone(), sample))
            }
            DriveSource::Health(name) => {
                let Some(disk) = disk_named(name) else {
                    return discover();
                };
                first_delivery::<DriveHealth>(DriveHealthInputs {
                    disk: disk.clone(),
                    sensor: drive_sensor.clone(),
                })
                .map(move |sample| DriveMessage::DriveHealth(disk.clone(), sample))
            }
        }
    }

    pub fn update(&mut self, msg: DriveMessage, clock: Clock) -> Task<DriveMessage> {
        match msg {
            DriveMessage::Discovered(sample) => {
                self.discovery.receive(sample, clock);
                if let Some(discovery) = self.discovery.get() {
                    self.disks = discovery
                        .whole_disks()
                        .into_iter()
                        .map(|disk| {
                            let samples = self.disks.remove(&disk).unwrap_or_default();
                            (disk, samples)
                        })
                        .collect();
                }
            }
            DriveMessage::DiskSpace(disk, sample) => {
                if let Some(samples) = self.disks.get_mut(&disk) {
                    samples.space.receive(*sample, clock);
                }
            }
            DriveMessage::DiskActivity(disk, sample) => {
                if let Some(samples) = self.disks.get_mut(&disk) {
                    samples.activity.receive(sample, clock);
                }
            }
            DriveMessage::DriveHealth(disk, sample) => {
                if let Some(samples) = self.disks.get_mut(&disk) {
                    samples.drive_health.receive(sample, clock);
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
                if let Some(samples) = self.disks.get_mut(&disk) {
                    samples.expansion.update(expansion);
                }
            }
            DriveInteraction::Refresh(source) => {
                return self.refresh(
                    &source,
                    &config.indicator_groups.physical_drive.temperature_sensors,
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

    fn mount_discovery_indicator<'view>(
        &'view self,
        discovery_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        discovery_row
            .with_hover_card(Box::new(move |_| {
                self.discovery
                    .sections_described_by(block_filesystem_discovery_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::MountDiscovery))
    }

    #[must_use]
    pub fn group_contents(&self, locale: Locale) -> Vec<IndicatorGroupContent<'_>> {
        let headline_indicator = self
            .mount_discovery_indicator(self.discovery.indicator(
                |held_snapshot| block_filesystem_discovery_indicator(held_snapshot),
                HARD_DRIVES_GLYPH.into(),
            ))
            .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into());
        let discovery_group = IndicatorGroupContent::expandable(
            IndicatorGroupFamily::PhysicalDrive,
            headline_indicator,
            &self.discovery_expansion,
            DriveInteraction::DiscoveryExpansion,
            self.discovery
                .indicator_listing(
                    |held_snapshot| vec![block_filesystem_discovery_indicator(held_snapshot)],
                    HARD_DRIVES_GLYPH.into(),
                )
                .map_indicators(|discovery_row| self.mount_discovery_indicator(discovery_row)),
        );
        [discovery_group]
            .into_iter()
            .chain(
                self.disks
                    .iter()
                    .map(|(disk, samples)| self.disk_group(disk, samples, locale)),
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

    use super::{IndicatorGroupFamily, disk_group_title};

    fn disk_named(name: &str) -> KernelDevice {
        KernelDevice {
            name: name.to_owned(),
        }
    }

    #[test]
    fn each_disk_gets_its_own_indicator_title() {
        let first_disk_title = disk_group_title(&disk_named("nvme0n1")).to_string();
        assert_eq!(
            first_disk_title,
            format!("{} nvme0n1", IndicatorGroupFamily::PhysicalDrive.title())
        );
        assert_ne!(
            disk_group_title(&disk_named("nvme0n1")),
            disk_group_title(&disk_named("sda"))
        );
    }
}
