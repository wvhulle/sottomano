use chrono::Locale;

mod bcachefs_member_indicator;
mod disk_space_gauge;
mod disk_transfer_gauge;
mod drive_health_indicator;

use std::collections::BTreeMap;

use bcachefs_member_indicator::BcachefsMemberIndicator;
use corredo::transitions::ExpansionState;
use disk_space_gauge::{DiskMarking, disk_space_hover_card, disk_space_indicator};
use disk_transfer_gauge::{
    disk_activity_indicator, disk_transfer_hover_card, disk_transfer_indicator,
};
use drive_health_indicator::{drive_health_hover_card, drive_health_indicator};
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
    group_indicators::{
        group_indicator::{GroupIndicator, PanelVisibility},
        indicator_state::IndicatorState,
    },
    group_menus::group_source_demand::GroupSourceDemand,
    widget_groups::{
        storage_space_group::{
            block_filesystem_discovery_hover_card, block_filesystem_discovery_indicator,
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
struct PhysicalDiskSamples {
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
    disks: BTreeMap<KernelDevice, PhysicalDiskSamples>,
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
            .map(|samples| samples.expansion.state())
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

    fn transfer_indicators<'a>(
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
    ) -> impl Iterator<Item = GroupIndicator<'a>> {
        samples
            .activity
            .get()
            .into_iter()
            .flat_map(move |activity| {
                [
                    disk_transfer_indicator(&activity.read)
                        .with_hover_card(Box::new(move |_| {
                            samples
                                .activity
                                .hover_card_with(disk_transfer_hover_card(&activity.read))
                        }))
                        .refreshed_by(refresh_activity(disk)),
                    disk_transfer_indicator(&activity.write)
                        .with_hover_card(Box::new(move |_| {
                            samples
                                .activity
                                .hover_card_with(disk_transfer_hover_card(&activity.write))
                        }))
                        .refreshed_by(refresh_activity(disk)),
                ]
            })
    }

    fn member_indicators<'a>(
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
        locale: Locale,
    ) -> impl Iterator<Item = GroupIndicator<'a>> {
        samples
            .space
            .get()
            .into_iter()
            .flat_map(|occupancy| &occupancy.filesystem_shares)
            .flat_map(|share| &share.bcachefs_members)
            .map(move |device| {
                BcachefsMemberIndicator { device }
                    .indicator(locale)
                    .with_hover_card(Box::new(move |_| {
                        samples
                            .space
                            .hover_card_with(BcachefsMemberIndicator { device }.hover_card())
                    }))
                    .refreshed_by(DriveInteraction::Refresh(DriveSource::Space(
                        disk.name.clone(),
                    )))
            })
    }

    fn disk_group<'a>(
        &'a self,
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'a> {
        let headline_indicator = self.disk_space_headline(disk, samples, clock);
        let mut rows: Vec<_> = Self::member_indicators(disk, samples, locale).collect();
        Self::push_drive_health_indicators(&mut rows, clock, disk, samples);
        Self::push_disk_activity_indicators(&mut rows, clock, disk, samples);
        rows.extend(Self::transfer_indicators(disk, samples));
        self.list_disk_space(disk, samples, clock, &mut rows);
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(WidgetGroupFamily::PhysicalDrive, disk.name.clone()),
            disk_group_title(disk),
            headline_indicator,
            &samples.expansion,
            move |expansion| DriveInteraction::DiskExpansion {
                disk: disk.clone(),
                expansion,
            },
            rows,
        )
    }

    fn disk_space_headline<'a>(
        &self,
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
        clock: Clock,
    ) -> GroupIndicator<'a> {
        match &samples.space {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                Self::wired_disk_space_indicator(
                    disk,
                    samples,
                    disk_space_indicator(held_snapshot, self.disk_marking(disk)),
                )
                .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into())
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                Self::wired_disk_space_indicator(
                    disk,
                    samples,
                    troubled_snapshot.shown_over(
                        disk_space_indicator(
                            &troubled_snapshot.held_snapshot,
                            self.disk_marking(disk),
                        ),
                        clock,
                    ),
                )
                .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into())
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => Self::wired_disk_space_indicator(
                disk,
                samples,
                placeholder.indicator(HARD_DRIVES_GLYPH.into()),
            )
            .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into()),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupIndicator::glyph_only(
                WidgetGroupFamily::PhysicalDrive.title(),
                Phosphor::Cylinder(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupIndicator::glyph_only(
                WidgetGroupFamily::PhysicalDrive.title(),
                Phosphor::Cylinder(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        }
    }

    fn list_disk_space<'a>(
        &self,
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
        clock: Clock,
        indicators: &mut Vec<GroupIndicator<'a>>,
    ) {
        match &samples.space {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => indicators.insert(
                0,
                Self::wired_disk_space_indicator(
                    disk,
                    samples,
                    disk_space_indicator(held_snapshot, self.disk_marking(disk)),
                ),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => indicators.insert(
                0,
                Self::wired_disk_space_indicator(
                    disk,
                    samples,
                    troubled_snapshot.shown_over(
                        disk_space_indicator(
                            &troubled_snapshot.held_snapshot,
                            self.disk_marking(disk),
                        ),
                        clock,
                    ),
                ),
            ),
            SubscribedSnapshot::NoSnapshot { placeholder } => indicators.insert(
                0,
                Self::wired_disk_space_indicator(
                    disk,
                    samples,
                    placeholder.indicator(HARD_DRIVES_GLYPH.into()),
                ),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_drive_health_indicators<'a>(
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
    ) {
        match &samples.drive_health {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let health_indicator = drive_health_indicator(held_snapshot);
                if health_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(Self::wired_drive_health_indicator(
                        disk,
                        samples,
                        health_indicator,
                    ));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let health_indicator = troubled_snapshot.shown_over(
                    drive_health_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if health_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(Self::wired_drive_health_indicator(
                        disk,
                        samples,
                        health_indicator,
                    ));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(Self::wired_drive_health_indicator(
                    disk,
                    samples,
                    placeholder.indicator(DRIVE_HEALTH_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn push_disk_activity_indicators<'a>(
        rows: &mut Vec<GroupIndicator<'a>>,
        clock: Clock,
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
    ) {
        match &samples.activity {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => {
                let activity_indicator = disk_activity_indicator(held_snapshot);
                if activity_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(Self::wired_disk_activity_indicator(
                        disk,
                        samples,
                        activity_indicator,
                    ));
                }
            }
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                let activity_indicator = troubled_snapshot.shown_over(
                    disk_activity_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                );
                if activity_indicator.panel_visibility() == PanelVisibility::Shown {
                    rows.push(Self::wired_disk_activity_indicator(
                        disk,
                        samples,
                        activity_indicator,
                    ));
                }
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => {
                rows.push(Self::wired_disk_activity_indicator(
                    disk,
                    samples,
                    placeholder.indicator(DISK_ACTIVITY_GLYPH.into()),
                ));
            }
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
    }

    fn wired_disk_space_indicator<'a>(
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
        space_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        space_indicator
            .with_hover_card(Box::new(move |_| {
                samples.space.sections_described_by(disk_space_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::Space(
                disk.name.clone(),
            )))
    }

    fn wired_drive_health_indicator<'a>(
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
        health_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        health_indicator
            .with_hover_card(Box::new(move |_| {
                samples
                    .drive_health
                    .sections_described_by(drive_health_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::Health(
                disk.name.clone(),
            )))
    }

    fn wired_disk_activity_indicator<'a>(
        disk: &'a KernelDevice,
        samples: &'a PhysicalDiskSamples,
        activity_indicator: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        activity_indicator
            .with_hover_card(Box::new(move |_| {
                samples
                    .activity
                    .sections_described_by(|snapshot| disk_transfer_hover_card(&snapshot.read))
            }))
            .refreshed_by(refresh_activity(disk))
    }

    fn mount_discovery_indicator<'a>(
        &'a self,
        discovery_row: GroupIndicator<'a>,
    ) -> GroupIndicator<'a> {
        discovery_row
            .with_hover_card(Box::new(move |_| {
                self.discovery
                    .sections_described_by(block_filesystem_discovery_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::MountDiscovery))
    }

    #[must_use]
    pub fn group_contents(&self, clock: Clock, locale: Locale) -> Vec<WidgetGroupContent<'_>> {
        let headline_indicator = match &self.discovery {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => self
                .mount_discovery_indicator(block_filesystem_discovery_indicator(held_snapshot))
                .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into()),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => self
                .mount_discovery_indicator(troubled_snapshot.shown_over(
                    block_filesystem_discovery_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                ))
                .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into()),
            SubscribedSnapshot::NoSnapshot { placeholder } => self
                .mount_discovery_indicator(placeholder.indicator(HARD_DRIVES_GLYPH.into()))
                .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into()),
            SubscribedSnapshot::AwaitingFirstSnapshot => GroupIndicator::glyph_only(
                WidgetGroupFamily::PhysicalDrive.title(),
                Phosphor::Cylinder(Weight::Regular).into(),
                IndicatorState::Pending,
            ),
            SubscribedSnapshot::SourceUnavailable => GroupIndicator::glyph_only(
                WidgetGroupFamily::PhysicalDrive.title(),
                Phosphor::Cylinder(Weight::Regular).into(),
                IndicatorState::Off,
            ),
        };
        let mut lead_indicators = Vec::new();
        match &self.discovery {
            SubscribedSnapshot::SnapshotHeld { held_snapshot, .. } => lead_indicators.push(
                self.mount_discovery_indicator(block_filesystem_discovery_indicator(held_snapshot)),
            ),
            SubscribedSnapshot::SnapshotTroubled { troubled_snapshot } => {
                lead_indicators.push(self.mount_discovery_indicator(troubled_snapshot.shown_over(
                    block_filesystem_discovery_indicator(&troubled_snapshot.held_snapshot),
                    clock,
                )));
            }
            SubscribedSnapshot::NoSnapshot { placeholder } => lead_indicators.push(
                self.mount_discovery_indicator(placeholder.indicator(HARD_DRIVES_GLYPH.into())),
            ),
            SubscribedSnapshot::AwaitingFirstSnapshot | SubscribedSnapshot::SourceUnavailable => {}
        }
        let discovery_group = WidgetGroupContent::expandable(
            WidgetGroupFamily::PhysicalDrive,
            headline_indicator,
            &self.discovery_expansion,
            DriveInteraction::DiscoveryExpansion,
            lead_indicators.into_iter().chain([]),
        );
        [discovery_group]
            .into_iter()
            .chain(
                self.disks
                    .iter()
                    .map(|(disk, samples)| self.disk_group(disk, samples, clock, locale)),
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
    fn each_disk_gets_its_own_indicator_title() {
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
