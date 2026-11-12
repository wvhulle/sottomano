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
    SnapshotDelivery,
    disk_storage::{
        self,
        bcachefs_filesystem_space::SnapshotDeletion,
        bcachefs_subvolume_usage::BcachefsSubvolumeUsage,
        block_filesystem_mounts::{BlockFilesystemDiscovery, FilesystemMount},
        mounted_filesystem_usage::{Filesystem, FilesystemUsage},
    },
};

pub use self::filesystem_property_rows::FilesystemProperty;
use self::{filesystem_usage_gauge::SnapshotDeletionRow, subvolume_usage_row::SubvolumeUsageRow};
use crate::{
    background_work::{first_delivery, snapshot_subscription},
    clock::Clock,
    group_items::{
        group_item::GroupItem,
        group_item_reading::{GroupItemReading, PanelVisibility},
    },
    hover_cards::{
        HoverTarget,
        hover_card_body::{HoverCardBody, hover_card_source_delivery::HoverCard},
    },
    widget_groups::{
        widget_group_content::{
            WidgetGroupContent, WidgetGroupExpansion, WidgetGroupExpansionMessage,
        },
        widget_group_identity::{WidgetGroupFamily, WidgetGroupIdentity, WidgetGroupTitle},
    },
    widgets::{
        icon_glyph::IconGlyph,
        latest_snapshot::LatestSnapshot,
        phosphor::{Phosphor, Weight},
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

impl GroupItemReading for BlockFilesystemDiscovery {
    fn name(&self) -> &'static str {
        "Mounted filesystems"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Folders(Weight::Regular).into()
    }

    fn panel_visibility(&self) -> PanelVisibility {
        PanelVisibility::Hidden
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary(self.filesystems.iter().map(|filesystem| {
            LabeledValue::new(
                filesystem.primary_mount_point().display().to_string(),
                filesystem.fs_type.clone(),
            )
        }))
    }
}

fn mount_group_title(primary_mount_point: &Path) -> WidgetGroupTitle {
    WidgetGroupTitle::Named(format!("Filesystem {}", primary_mount_point.display()).into())
}

struct MountedFilesystem {
    mount: FilesystemMount,
    usage: LatestSnapshot<FilesystemUsage>,
    subvolume_usage: LatestSnapshot<BcachefsSubvolumeUsage>,
    expansion: WidgetGroupExpansion,
}

#[derive(Default)]
pub struct StorageSpaceGroup {
    discovery: LatestSnapshot<BlockFilesystemDiscovery>,
    filesystems: BTreeMap<PathBuf, MountedFilesystem>,
    discovery_expansion: WidgetGroupExpansion,
}

#[derive(Debug, Clone)]
pub enum StorageMessage {
    Discovered(SnapshotDelivery<BlockFilesystemDiscovery>),
    Usage(PathBuf, Box<SnapshotDelivery<FilesystemUsage>>),
    SubvolumeUsage(PathBuf, Box<SnapshotDelivery<BcachefsSubvolumeUsage>>),
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

    pub fn update(&mut self, msg: StorageMessage, clock: Clock) -> Task<StorageMessage> {
        match msg {
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
                                    usage: LatestSnapshot::default(),
                                    subvolume_usage: LatestSnapshot::default(),
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
            StorageMessage::DiscoveryExpansion(expansion) => {
                self.discovery_expansion.update(expansion);
            }
            StorageMessage::MountExpansion {
                mount_point,
                expansion,
            } => {
                if let Some(filesystem) = self.filesystems.get_mut(&mount_point) {
                    filesystem.expansion.update(expansion);
                }
            }
        }
        Task::none()
    }

    fn mount_group(filesystem: &MountedFilesystem) -> WidgetGroupContent<'_, StorageMessage> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        let usage = filesystem.usage.get();
        let property_items = usage.into_iter().flat_map(move |usage| {
            FilesystemProperty::listed_for(usage)
                .into_iter()
                .filter_map(move |property| {
                    let row = property.row(usage)?;
                    Some(row.group_item(StorageTarget::Property {
                        filesystem: primary_mount_point.to_path_buf(),
                        property,
                    }))
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
                        },
                        StorageTarget::Subvolume {
                            filesystem: primary_mount_point.to_path_buf(),
                            subvolume_path: subvolume.path.clone(),
                        },
                    )
                })
            });
        let subvolume_failure_item = filesystem.subvolume_usage.failure().map(|failure| {
            GroupItem::reading(
                failure,
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
                        SnapshotDeletionRow,
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
            move |expansion| StorageMessage::MountExpansion {
                mount_point: owned_mount_point.clone(),
                expansion,
            },
            filesystem
                .usage
                .group_item(StorageTarget::Usage(primary_mount_point.to_path_buf()))
                .into_iter()
                .chain(property_items)
                .chain(subvolume_items)
                .chain(subvolume_failure_item)
                .chain(snapshot_deletion_item),
        )
    }

    #[must_use]
    pub fn group_contents(&self) -> Vec<WidgetGroupContent<'_, StorageMessage>> {
        if !self.filesystems.is_empty() {
            return self.filesystems.values().map(Self::mount_group).collect();
        }
        vec![WidgetGroupContent::expandable(
            WidgetGroupFamily::StorageSpace,
            &self.discovery_expansion,
            StorageMessage::DiscoveryExpansion,
            self.discovery.group_item(StorageTarget::MountDiscovery),
        )]
    }

    #[must_use]
    pub fn hover_card_for(&self, target: &StorageTarget, clock: Clock) -> Option<HoverCard> {
        match target {
            StorageTarget::MountDiscovery => self.discovery.hover_card(clock),
            StorageTarget::Usage(mount_point) => {
                self.filesystems.get(mount_point)?.usage.hover_card(clock)
            }
            StorageTarget::Property {
                filesystem,
                property,
            } => {
                let filesystem = self.filesystems.get(filesystem)?;
                let row = property.row(filesystem.usage.get()?)?;
                filesystem.usage.hover_card_with(row.hover_card(clock))
            }
            StorageTarget::SubvolumeUsage(mount_point) => self
                .filesystems
                .get(mount_point)?
                .subvolume_usage
                .hover_card(clock),
            StorageTarget::Subvolume {
                filesystem,
                subvolume_path,
            } => {
                let filesystem = self.filesystems.get(filesystem)?;
                let subvolume = filesystem
                    .subvolume_usage
                    .get()?
                    .subvolumes
                    .iter()
                    .find(|known| known.path == *subvolume_path)?;
                filesystem.subvolume_usage.hover_card_with(
                    SubvolumeUsageRow {
                        subvolume,
                        pool_total: filesystem.usage.get()?.total,
                    }
                    .hover_card(clock),
                )
            }
            StorageTarget::SnapshotDeletion(mount_point) => {
                let filesystem = self.filesystems.get(mount_point)?;
                filesystem
                    .usage
                    .get()
                    .filter(|usage| {
                    matches!(
                        &usage.filesystem,
                        Filesystem::Bcachefs(space) if space.snapshot_deletion == SnapshotDeletion::Running
                    )
                })?;
                filesystem
                    .usage
                    .hover_card_with(SnapshotDeletionRow.hover_card(clock))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{StorageTarget, mount_group_title};
    use crate::hover_cards::HoverTarget;

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

    #[test]
    fn subvolume_rows_pin_separately_by_path() {
        let pin_key_of = |subvolume_path: &str| {
            HoverTarget::from(StorageTarget::Subvolume {
                filesystem: PathBuf::from("/"),
                subvolume_path: PathBuf::from(subvolume_path),
            })
            .stable_item_key()
        };
        assert_ne!(pin_key_of("/home"), pin_key_of("/nix"));
        assert_eq!(pin_key_of("/home"), pin_key_of("/home"));
    }
}
