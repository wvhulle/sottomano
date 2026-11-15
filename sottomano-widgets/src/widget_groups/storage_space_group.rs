use chrono::Locale;
mod compression_setting_row;
mod ext4_error_count_row;
mod ext4_lifetime_writes_row;
mod filesystem_property_rows;
mod filesystem_usage_gauge;
mod inode_usage_row;
mod metadata_room_row;
mod mount_access_row;
mod subvolume_usage_row;
mod volume_identity_row;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use iced::{Subscription, Task};
use sottomano_desktop_sources::{
    SnapshotDelivery, SnapshotSource,
    disk_storage::{
        bcachefs_filesystem_space::SnapshotDeletion,
        bcachefs_subvolume_usage::BcachefsSubvolumeUsage,
        block_filesystem_mounts::{BlockFilesystemDiscovery, FilesystemMount},
        mounted_filesystem_usage::{Filesystem, FilesystemUsage},
    },
};

pub use self::filesystem_property_rows::FilesystemProperty;
use self::{
    filesystem_usage_gauge::{
        SnapshotDeletionRow, filesystem_usage_hover_card, filesystem_usage_reading_row,
    },
    subvolume_usage_row::{SubvolumeUsageRow, subvolume_usage_hover_card},
};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        reading_display_state::ReadingDisplayState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{Section, dictionary_table},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        icon_glyph::IconGlyph,
        phosphor::{Phosphor, Weight},
        subscribed_snapshot::SubscribedSnapshot,
        value_cells::LabeledValue,
    },
};

const BCACHEFS_FS_TYPE: &str = "bcachefs";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageTarget {
    MountDiscovery,
    Usage(PathBuf),
    Property {
        filesystem: PathBuf,
        property: FilesystemProperty,
    },
    SubvolumeUsage(PathBuf),
    Subvolume {
        filesystem: PathBuf,
        subvolume_path: PathBuf,
    },
    SnapshotDeletion(PathBuf),
}

impl From<StorageTarget> for HoverTarget {
    fn from(target: StorageTarget) -> Self {
        HoverTarget::Storage(target)
    }
}

pub(crate) fn block_filesystem_discovery_reading_row(
    discovery: &BlockFilesystemDiscovery,
) -> ReadingRow<'_> {
    ReadingRow {
        panel_visibility: PanelVisibility::Hidden,
        ..ReadingRow::glyph_only(
            BlockFilesystemDiscovery::NAME,
            Phosphor::Folders(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(move |_| block_filesystem_discovery_hover_card(discovery)),
        )
    }
}

pub(crate) fn block_filesystem_discovery_hover_card(
    discovery: &BlockFilesystemDiscovery,
) -> Vec<Section> {
    vec![Section::new(
        "Mount points",
        dictionary_table(discovery.filesystems.iter().map(|filesystem| {
            LabeledValue::new(
                filesystem.primary_mount_point().display().to_string(),
                filesystem.fs_type.clone(),
            )
        })),
    )]
}

fn mount_group_title(primary_mount_point: &Path) -> WidgetGroupTitle {
    WidgetGroupTitle::Named(format!("Filesystem {}", primary_mount_point.display()).into())
}

struct MountedFilesystem {
    mount: FilesystemMount,
    usage: SubscribedSnapshot<FilesystemUsage>,
    subvolume_usage: SubscribedSnapshot<BcachefsSubvolumeUsage>,
    expansion: WidgetGroupExpansion,
}

#[derive(Default)]
pub struct StorageSpaceGroup {
    discovery: SubscribedSnapshot<BlockFilesystemDiscovery>,
    filesystems: BTreeMap<PathBuf, MountedFilesystem>,
    discovery_expansion: WidgetGroupExpansion,
}

#[derive(Debug)]
pub enum StorageMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    Usage(PathBuf, Box<SnapshotDelivery<FilesystemUsage>>),
    SubvolumeUsage(PathBuf, Box<SnapshotDelivery<BcachefsSubvolumeUsage>>),
    Interaction(StorageInteraction),
}

#[derive(Debug, Clone)]
pub enum StorageInteraction {
    DiscoveryExpansion(WidgetGroupExpansionMessage),
    MountExpansion {
        mount_point: PathBuf,
        expansion: WidgetGroupExpansionMessage,
    },
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
                .map(|(mount_point, reading)| StorageMessage::Usage(mount_point, Box::new(reading)))
        });
        let subvolume_usages = self
            .filesystems
            .iter()
            .filter(|(_, filesystem)| filesystem.mount.fs_type == BCACHEFS_FS_TYPE)
            .map(|(mount_point, filesystem)| {
                snapshot_subscription::<BcachefsSubvolumeUsage>(filesystem.mount.clone())
                    .with(mount_point.clone())
                    .map(|(mount_point, reading)| {
                        StorageMessage::SubvolumeUsage(mount_point, Box::new(reading))
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

    pub fn refresh(&self, target: &StorageTarget) -> Task<StorageMessage> {
        let discover =
            || first_delivery::<BlockFilesystemDiscovery>(()).map(StorageMessage::Discovered);
        match target {
            StorageTarget::MountDiscovery => discover(),
            StorageTarget::Usage(primary_mount_point)
            | StorageTarget::Property {
                filesystem: primary_mount_point,
                ..
            }
            | StorageTarget::SnapshotDeletion(primary_mount_point) => {
                let Some(filesystem) = self.filesystems.get(primary_mount_point) else {
                    return discover();
                };
                let primary_mount_point = primary_mount_point.clone();
                first_delivery::<FilesystemUsage>(filesystem.mount.clone()).map(move |reading| {
                    StorageMessage::Usage(primary_mount_point.clone(), Box::new(reading))
                })
            }
            StorageTarget::SubvolumeUsage(primary_mount_point)
            | StorageTarget::Subvolume {
                filesystem: primary_mount_point,
                ..
            } => {
                let Some(filesystem) = self.filesystems.get(primary_mount_point) else {
                    return discover();
                };
                let primary_mount_point = primary_mount_point.clone();
                first_delivery::<BcachefsSubvolumeUsage>(filesystem.mount.clone()).map(
                    move |reading| {
                        StorageMessage::SubvolumeUsage(
                            primary_mount_point.clone(),
                            Box::new(reading),
                        )
                    },
                )
            }
        }
    }

    pub fn update(&mut self, message: StorageMessage, clock: Clock) -> Task<StorageMessage> {
        match message {
            StorageMessage::Discovered(reading) => {
                self.discovery.receive(reading, clock);
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
            StorageMessage::Usage(mount_point, reading) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.usage.receive(*reading, clock);
                }
            }
            StorageMessage::SubvolumeUsage(mount_point, reading) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.subvolume_usage.receive(*reading, clock);
                }
            }
            StorageMessage::Interaction(StorageInteraction::DiscoveryExpansion(expansion)) => {
                self.discovery_expansion.update(expansion);
            }
            StorageMessage::Interaction(StorageInteraction::MountExpansion {
                mount_point,
                expansion,
            }) => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.expansion.update(expansion);
                }
            }
        }
        Task::none()
    }

    fn mount_group(
        filesystem: &MountedFilesystem,
        clock: Clock,
        locale: Locale,
    ) -> WidgetGroupContent<'_, StorageInteraction> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        let usage = filesystem.usage.get();
        let property_items = usage.into_iter().flat_map(move |usage| {
            FilesystemProperty::listed_for(usage)
                .into_iter()
                .filter_map(move |property| {
                    let row = property.row(usage)?;
                    Some(row.group_item(
                        StorageTarget::Property {
                            filesystem: primary_mount_point.to_path_buf(),
                            property,
                        },
                        locale,
                    ))
                })
        });
        let subvolume_items = usage
            .zip(filesystem.subvolume_usage.get())
            .into_iter()
            .flat_map(move |(usage, subvolume_usage)| {
                subvolume_usage.subvolumes.iter().map(move |subvolume| {
                    GroupItem::reading(
                        SubvolumeUsageRow {
                            subvolume,
                            pool_total: usage.total,
                        }
                        .reading_row(locale),
                        StorageTarget::Subvolume {
                            filesystem: primary_mount_point.to_path_buf(),
                            subvolume_path: subvolume.path.clone(),
                        },
                    )
                })
            });
        let subvolume_failure_item = filesystem.subvolume_usage.failure().map(|failure| {
            GroupItem::reading(
                failure.reading_row(IconGlyph::default()),
                StorageTarget::SubvolumeUsage(primary_mount_point.to_path_buf()),
            )
        });
        let snapshot_deletion_item =
            usage
                .filter(|usage| {
                    matches!(
                        &usage.filesystem,
                        Filesystem::Bcachefs(space) if space.snapshot_deletion == SnapshotDeletion::Running
                    )
                })
                .map(|_| {
                    GroupItem::reading(
                        SnapshotDeletionRow::reading_row(),
                        StorageTarget::SnapshotDeletion(primary_mount_point.to_path_buf()),
                    )
                });
        let owned_mount_point = primary_mount_point.to_path_buf();
        WidgetGroupContent::expandable_instance(
            WidgetGroupIdentity::durable(
                WidgetGroupFamily::StorageSpace,
                primary_mount_point.display().to_string(),
            ),
            mount_group_title(primary_mount_point),
            &filesystem.expansion,
            move |expansion| StorageInteraction::MountExpansion {
                mount_point: owned_mount_point.clone(),
                expansion,
            },
            filesystem
                .usage
                .group_item(
                    IconGlyph::default(),
                    |snapshot| filesystem_usage_reading_row(snapshot),
                    StorageTarget::Usage(primary_mount_point.to_path_buf()),
                    clock,
                )
                .into_iter()
                .chain(property_items)
                .chain(subvolume_items)
                .chain(subvolume_failure_item)
                .chain(snapshot_deletion_item),
        )
    }

    #[must_use]
    pub fn group_contents(
        &self,
        clock: Clock,
        locale: Locale,
    ) -> Vec<WidgetGroupContent<'_, StorageInteraction>> {
        if !self.filesystems.is_empty() {
            return self
                .filesystems
                .values()
                .map(|filesystem| Self::mount_group(filesystem, clock, locale))
                .collect();
        }
        vec![WidgetGroupContent::expandable(
            WidgetGroupFamily::StorageSpace,
            &self.discovery_expansion,
            StorageInteraction::DiscoveryExpansion,
            self.discovery.group_item(
                IconGlyph::default(),
                |snapshot| block_filesystem_discovery_reading_row(snapshot),
                StorageTarget::MountDiscovery,
                clock,
            ),
        )]
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &StorageTarget) -> Vec<Section> {
        match target {
            StorageTarget::MountDiscovery => self
                .discovery
                .sections_described_by(block_filesystem_discovery_hover_card),
            StorageTarget::Usage(mount_point) => self
                .filesystems
                .get(mount_point)
                .map(|filesystem| {
                    filesystem
                        .usage
                        .sections_described_by(filesystem_usage_hover_card)
                })
                .unwrap_or_default(),
            StorageTarget::Property {
                filesystem,
                property,
            } => {
                let Some(filesystem) = self.filesystems.get(filesystem) else {
                    return Vec::new();
                };
                let Some(row) = filesystem.usage.get().and_then(|usage| property.row(usage)) else {
                    return Vec::new();
                };
                filesystem.usage.hover_card_with(row.hover_card())
            }
            StorageTarget::SubvolumeUsage(mount_point) => self
                .filesystems
                .get(mount_point)
                .map(|filesystem| {
                    filesystem
                        .subvolume_usage
                        .sections_described_by(subvolume_usage_hover_card)
                })
                .unwrap_or_default(),
            StorageTarget::Subvolume {
                filesystem,
                subvolume_path,
            } => {
                let Some(filesystem) = self.filesystems.get(filesystem) else {
                    return Vec::new();
                };
                let Some(subvolume) =
                    filesystem
                        .subvolume_usage
                        .get()
                        .and_then(|subvolume_usage| {
                            subvolume_usage
                                .subvolumes
                                .iter()
                                .find(|known| known.path == *subvolume_path)
                        })
                else {
                    return Vec::new();
                };
                let Some(usage) = filesystem.usage.get() else {
                    return Vec::new();
                };
                filesystem.subvolume_usage.hover_card_with(
                    SubvolumeUsageRow {
                        subvolume,
                        pool_total: usage.total,
                    }
                    .hover_card(),
                )
            }
            StorageTarget::SnapshotDeletion(mount_point) => {
                let Some(filesystem) = self.filesystems.get(mount_point) else {
                    return Vec::new();
                };
                let deletion_running = filesystem.usage.get().is_some_and(|usage| {
                    matches!(
                        &usage.filesystem,
                        Filesystem::Bcachefs(space) if space.snapshot_deletion == SnapshotDeletion::Running
                    )
                });
                if !deletion_running {
                    return Vec::new();
                }
                filesystem
                    .usage
                    .hover_card_with(SnapshotDeletionRow::hover_card())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::mount_group_title;

    #[test]
    fn each_filesystem_gets_its_own_row_titled_by_its_mount_point() {
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
