use chrono::Locale;

mod filesystem_property_indicators;
mod filesystem_usage_indicator;
mod mounted_filesystem_group;
mod snapshot_deletion_indicator;
mod subvolume_usage_indicator;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery,
    disk_storage::{
        bcachefs_subvolume_usage::BcachefsSubvolumeUsage,
        block_filesystem_mounts::{BlockFilesystemDiscovery, FilesystemMount},
        mounted_filesystem_usage::FilesystemUsage,
    },
};

pub use self::filesystem_property_indicators::FilesystemProperty;
use super::block_filesystem_discovery_indicator::{
    block_filesystem_discovery_hover_card, block_filesystem_discovery_indicator,
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{
            IndicatorGroupContent, IndicatorGroupExpansion, IndicatorGroupExpansionMessage,
        },
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupTitle},
    },
    widgets::subscribed_snapshot::SubscribedSnapshot,
};

const BCACHEFS_FS_TYPE: &str = "bcachefs";
const FILESYSTEM_USAGE_GLYPH: Phosphor = Phosphor::Folders(Weight::Regular);
const SUBVOLUME_USAGE_GLYPH: Phosphor = Phosphor::FolderSimple(Weight::Regular);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageSource {
    MountDiscovery,
    Usage(PathBuf),
    SubvolumeUsage(PathBuf),
}

fn mount_group_title(primary_mount_point: &Path) -> IndicatorGroupTitle {
    IndicatorGroupTitle::Named(format!("Filesystem {}", primary_mount_point.display()).into())
}

struct MountedFilesystem {
    mount: FilesystemMount,
    usage: SubscribedSnapshot<FilesystemUsage>,
    subvolume_usage: SubscribedSnapshot<BcachefsSubvolumeUsage>,
    expansion: IndicatorGroupExpansion,
}

#[derive(Default)]
pub struct StorageSpaceGroup {
    discovery: SubscribedSnapshot<BlockFilesystemDiscovery>,
    filesystems: BTreeMap<PathBuf, MountedFilesystem>,
    discovery_expansion: IndicatorGroupExpansion,
}

#[derive(Debug)]
pub enum StorageMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    Usage(PathBuf, Box<SnapshotDelivery<FilesystemUsage>>),
    SubvolumeUsage(PathBuf, Box<SnapshotDelivery<BcachefsSubvolumeUsage>>),
}

#[derive(Debug, Clone)]
pub enum StorageInteraction {
    DiscoveryExpansion(IndicatorGroupExpansionMessage),
    MountExpansion {
        mount_point: PathBuf,
        expansion: IndicatorGroupExpansionMessage,
    },
    Refresh(StorageSource),
}

impl StorageSpaceGroup {
    pub fn collapse(&mut self) {
        self.discovery_expansion.collapse();
        self.filesystems.values_mut().for_each(|filesystem| {
            filesystem.expansion.collapse();
        });
    }

    pub fn subscription(&self) -> Subscription<StorageMessage> {
        let usages = self.filesystems.iter().map(|(mount_point, filesystem)| {
            snapshot_subscription::<FilesystemUsage>(filesystem.mount.clone())
                .with(mount_point.clone())
                .map(|(mount_point, sample)| StorageMessage::Usage(mount_point, Box::new(sample)))
        });
        let subvolume_usages = self
            .filesystems
            .iter()
            .filter(|(_, filesystem)| filesystem.mount.fs_type == BCACHEFS_FS_TYPE)
            .map(|(mount_point, filesystem)| {
                snapshot_subscription::<BcachefsSubvolumeUsage>(filesystem.mount.clone())
                    .with(mount_point.clone())
                    .map(|(mount_point, sample)| {
                        StorageMessage::SubvolumeUsage(mount_point, Box::new(sample))
                    })
            });
        Subscription::batch(
            [
                snapshot_subscription::<BlockFilesystemDiscovery>(())
                    .map(StorageMessage::Discovered),
            ]
            .into_iter()
            .chain(usages)
            .chain(subvolume_usages),
        )
    }

    fn refresh(&self, source: StorageSource) -> Task<StorageMessage> {
        let discover =
            || first_delivery::<BlockFilesystemDiscovery>(()).map(StorageMessage::Discovered);
        match source {
            StorageSource::MountDiscovery => discover(),
            StorageSource::Usage(primary_mount_point) => {
                let Some(filesystem) = self.filesystems.get(&primary_mount_point) else {
                    return discover();
                };
                first_delivery::<FilesystemUsage>(filesystem.mount.clone()).map(move |sample| {
                    StorageMessage::Usage(primary_mount_point.clone(), Box::new(sample))
                })
            }
            StorageSource::SubvolumeUsage(primary_mount_point) => {
                let Some(filesystem) = self.filesystems.get(&primary_mount_point) else {
                    return discover();
                };
                first_delivery::<BcachefsSubvolumeUsage>(filesystem.mount.clone()).map(
                    move |sample| {
                        StorageMessage::SubvolumeUsage(
                            primary_mount_point.clone(),
                            Box::new(sample),
                        )
                    },
                )
            }
        }
    }

    pub fn update(&mut self, message: StorageMessage, clock: Clock) -> Task<StorageMessage> {
        match message {
            StorageMessage::Discovered(sample) => {
                self.discovery.receive(sample, clock);
                if let Some(discovery) = self.discovery.get() {
                    self.filesystems = discovery
                        .filesystems
                        .iter()
                        .map(|mount| {
                            let mount_point = mount.primary_mount_point().to_path_buf();
                            let filesystem = match self.filesystems.remove(&mount_point) {
                                Some(known) if &known.mount == mount => known,
                                known => MountedFilesystem {
                                    mount: mount.clone(),
                                    usage: SubscribedSnapshot::default(),
                                    subvolume_usage: SubscribedSnapshot::default(),
                                    expansion: known
                                        .map(|known| known.expansion)
                                        .unwrap_or_default(),
                                },
                            };
                            (mount_point, filesystem)
                        })
                        .collect();
                }
            }
            StorageMessage::Usage(mount_point, sample) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.usage.receive(*sample, clock);
                }
            }
            StorageMessage::SubvolumeUsage(mount_point, sample) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.subvolume_usage.receive(*sample, clock);
                }
            }
        }
        Task::none()
    }

    pub fn interact(&mut self, interaction: StorageInteraction) -> Task<StorageMessage> {
        match interaction {
            StorageInteraction::DiscoveryExpansion(expansion) => {
                self.discovery_expansion.update(expansion);
            }
            StorageInteraction::MountExpansion {
                mount_point,
                expansion,
            } => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.expansion.update(expansion);
                }
            }
            StorageInteraction::Refresh(source) => {
                return self.refresh(source);
            }
        }
        Task::none()
    }

    fn discovery_indicator<'view>(
        &'view self,
        discovery_row: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        discovery_row
            .with_hover_card(Box::new(move |_| {
                self.discovery
                    .sections_described_by(block_filesystem_discovery_hover_card)
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::MountDiscovery))
    }

    #[must_use]
    pub fn group_contents(&self, locale: Locale) -> Vec<IndicatorGroupContent<'_>> {
        if !self.filesystems.is_empty() {
            return self
                .filesystems
                .values()
                .map(|filesystem| Self::mount_group(filesystem, locale))
                .collect();
        }
        let headline_indicator = self
            .discovery_indicator(self.discovery.indicator(
                |held_snapshot| block_filesystem_discovery_indicator(held_snapshot),
                FILESYSTEM_USAGE_GLYPH.into(),
            ))
            .with_headline_glyph(Phosphor::Archive(Weight::Regular).into());
        vec![IndicatorGroupContent::expandable(
            IndicatorGroupFamily::StorageSpace,
            headline_indicator,
            &self.discovery_expansion,
            StorageInteraction::DiscoveryExpansion,
            self.discovery
                .indicator_listing(
                    |held_snapshot| vec![block_filesystem_discovery_indicator(held_snapshot)],
                    FILESYSTEM_USAGE_GLYPH.into(),
                )
                .map_indicators(|discovery_row| self.discovery_indicator(discovery_row)),
        )]
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::mount_group_title;

    #[test]
    fn each_filesystem_gets_its_own_indicator_titled_by_its_mount_point() {
        assert_eq!(
            mount_group_title(Path::new("/home")).to_string(),
            "Filesystem /home"
        );
        assert_ne!(
            mount_group_title(Path::new("/")),
            mount_group_title(Path::new("/home"))
        );
    }
}
