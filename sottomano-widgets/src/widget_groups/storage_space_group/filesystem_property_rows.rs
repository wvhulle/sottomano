use sottomano_desktop_sources::disk_storage::mounted_filesystem_usage::{
    Filesystem, FilesystemUsage,
};

use super::{
    bcachefs_member_row::BcachefsMemberRow, compression_setting_row::CompressionSettingRow,
    ext4_error_count_row::Ext4ErrorCountRow, ext4_lifetime_writes_row::Ext4LifetimeWritesRow,
    inode_usage_row::InodeUsageRow, metadata_room_row::MetadataRoomRow,
    mount_access_row::MountAccessRow, volume_identity_row::VolumeIdentityRow,
};
use crate::{
    clock::Clock,
    group_items::{group_item::GroupItem, group_item_reading::GroupItemReading},
    hover_cards::{HoverTarget, hover_card_body::HoverCardBody},
};

const SHARED_PROPERTIES: [FilesystemProperty; 7] = [
    FilesystemProperty::Mount,
    FilesystemProperty::Volume,
    FilesystemProperty::Inodes,
    FilesystemProperty::MetadataRoom,
    FilesystemProperty::Compression,
    FilesystemProperty::Ext4Errors,
    FilesystemProperty::Ext4Writes,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemProperty {
    Mount,
    Volume,
    Inodes,
    MetadataRoom,
    Compression,
    Member(String),
    Ext4Errors,
    Ext4Writes,
}

impl FilesystemProperty {
    pub(super) fn listed_for(usage: &FilesystemUsage) -> Vec<FilesystemProperty> {
        let members = match &usage.filesystem {
            Filesystem::Bcachefs(space) => space
                .devices
                .iter()
                .map(|device| FilesystemProperty::Member(device.partition.clone()))
                .collect(),
            Filesystem::Btrfs(_)
            | Filesystem::Ext4 { .. }
            | Filesystem::Xfs { .. }
            | Filesystem::Other { .. } => Vec::new(),
        };
        SHARED_PROPERTIES
            .into_iter()
            .chain(members)
            .filter(|property| property.row(usage).is_some())
            .collect()
    }

    pub(super) fn row<'a>(&self, usage: &'a FilesystemUsage) -> Option<FilesystemPropertyRow<'a>> {
        match (self, &usage.filesystem) {
            (FilesystemProperty::Mount, _) => {
                Some(FilesystemPropertyRow::Mount(MountAccessRow { usage }))
            }
            (FilesystemProperty::Volume, _) if !usage.volume.properties.is_empty() => {
                Some(FilesystemPropertyRow::Volume(VolumeIdentityRow { usage }))
            }
            (
                FilesystemProperty::Inodes,
                Filesystem::Ext4 { inodes, .. } | Filesystem::Xfs { inodes },
            ) if inodes.total > 0 => Some(FilesystemPropertyRow::Inodes(InodeUsageRow {
                inodes: *inodes,
            })),
            (FilesystemProperty::MetadataRoom, Filesystem::Btrfs(space)) => Some(
                FilesystemPropertyRow::MetadataRoom(MetadataRoomRow::Btrfs(space)),
            ),
            (FilesystemProperty::MetadataRoom, Filesystem::Bcachefs(space)) => Some(
                FilesystemPropertyRow::MetadataRoom(MetadataRoomRow::Bcachefs(space)),
            ),
            (FilesystemProperty::Compression, Filesystem::Btrfs(space)) => {
                enabled_compression(CompressionSettingRow::Btrfs(&space.compression))
            }
            (FilesystemProperty::Compression, Filesystem::Bcachefs(space)) => {
                enabled_compression(CompressionSettingRow::Bcachefs(space))
            }
            (FilesystemProperty::Member(partition), Filesystem::Bcachefs(space)) => space
                .devices
                .iter()
                .find(|device| &device.partition == partition)
                .map(|device| FilesystemPropertyRow::Member(BcachefsMemberRow { device })),
            (FilesystemProperty::Ext4Errors, Filesystem::Ext4 { counters, .. }) => Some(
                FilesystemPropertyRow::Ext4Errors(Ext4ErrorCountRow {
                    recorded_errors: counters.recorded_errors,
                }),
            ),
            (FilesystemProperty::Ext4Writes, Filesystem::Ext4 { counters, .. }) => Some(
                FilesystemPropertyRow::Ext4Writes(Ext4LifetimeWritesRow {
                    counters: *counters,
                }),
            ),
            _ => None,
        }
    }
}

fn enabled_compression(
    compression_row: CompressionSettingRow<'_>,
) -> Option<FilesystemPropertyRow<'_>> {
    compression_row
        .is_enabled()
        .then_some(FilesystemPropertyRow::Compression(compression_row))
}

pub(super) enum FilesystemPropertyRow<'a> {
    Mount(MountAccessRow<'a>),
    Volume(VolumeIdentityRow<'a>),
    Inodes(InodeUsageRow),
    MetadataRoom(MetadataRoomRow<'a>),
    Compression(CompressionSettingRow<'a>),
    Member(BcachefsMemberRow<'a>),
    Ext4Errors(Ext4ErrorCountRow),
    Ext4Writes(Ext4LifetimeWritesRow),
}

impl<'a> FilesystemPropertyRow<'a> {
    pub(super) fn group_item<M: 'a>(self, target: impl Into<HoverTarget>) -> GroupItem<'a, M> {
        match self {
            FilesystemPropertyRow::Mount(row) => GroupItem::reading(row, target),
            FilesystemPropertyRow::Volume(row) => GroupItem::reading(row, target),
            FilesystemPropertyRow::Inodes(row) => GroupItem::reading(row, target),
            FilesystemPropertyRow::MetadataRoom(row) => GroupItem::reading(row, target),
            FilesystemPropertyRow::Compression(row) => GroupItem::reading(row, target),
            FilesystemPropertyRow::Member(row) => GroupItem::reading(row, target),
            FilesystemPropertyRow::Ext4Errors(row) => GroupItem::reading(row, target),
            FilesystemPropertyRow::Ext4Writes(row) => GroupItem::reading(row, target),
        }
    }

    pub(super) fn hover_card(&self, clock: Clock) -> HoverCardBody {
        match self {
            FilesystemPropertyRow::Mount(row) => row.hover_card(clock),
            FilesystemPropertyRow::Volume(row) => row.hover_card(clock),
            FilesystemPropertyRow::Inodes(row) => row.hover_card(clock),
            FilesystemPropertyRow::MetadataRoom(row) => row.hover_card(clock),
            FilesystemPropertyRow::Compression(row) => row.hover_card(clock),
            FilesystemPropertyRow::Member(row) => row.hover_card(clock),
            FilesystemPropertyRow::Ext4Errors(row) => row.hover_card(clock),
            FilesystemPropertyRow::Ext4Writes(row) => row.hover_card(clock),
        }
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;
    use chrono::DateTime;
    use sottomano_desktop_sources::disk_storage::{
        bcachefs_filesystem_space::{
            BcachefsDeviceSpace, BcachefsSpace, CompressedExtents, SnapshotDeletion,
        },
        block_filesystem_mounts::{Compression, FilesystemMount},
        btrfs_filesystem_space::{Allocation, BtrfsSpace},
        ext4_sysfs_counters::{Ext4Counters, RecordedErrors},
        mounted_filesystem_usage::{InodeCount, MountAccess},
        udev_volume_identity::{VolumeIdentity, VolumeProperty, VolumePropertyKind},
    };

    use super::*;

    const PLENTY_OF_INODES: InodeCount = InodeCount {
        total: 1_000_000,
        free: 900_000,
    };

    fn usage_of(fs_type: &str, volume: VolumeIdentity, filesystem: Filesystem) -> FilesystemUsage {
        FilesystemUsage {
            mount: FilesystemMount {
                devices: Vec::new(),
                mount_points: vec!["/mnt".into()],
                fs_type: fs_type.to_owned(),
                compression: Compression::Disabled,
            },
            total: ByteSize::gib(100),
            free: ByteSize::gib(50),
            block_size: ByteSize::kib(4),
            access: MountAccess::ReadWrite,
            volume,
            filesystem,
        }
    }

    fn fat32_volume() -> VolumeIdentity {
        VolumeIdentity {
            properties: vec![VolumeProperty {
                kind: VolumePropertyKind::Version,
                value: "FAT32".to_owned(),
            }],
        }
    }

    fn bcachefs_space() -> BcachefsSpace {
        let member = |partition: &str| BcachefsDeviceSpace {
            label: partition.to_owned(),
            partition: partition.to_owned(),
            state: "rw".to_owned(),
            free: ByteSize::gib(10),
            capacity: ByteSize::gib(100),
            holds_metadata: true,
        };
        BcachefsSpace {
            devices: vec![member("nvme0n1p1"), member("nvme1n1p3")],
            metadata_replicas: 2,
            data_replicas: 1,
            compression: "zstd".to_owned(),
            background_compression: "none".to_owned(),
            compressed_extents: CompressedExtents::default(),
            snapshot_deletion: SnapshotDeletion::Idle,
        }
    }

    fn every_filesystem_kind() -> Vec<FilesystemUsage> {
        let ext4_counters = Ext4Counters {
            recorded_errors: RecordedErrors::Recorded {
                count: 1,
                first_seen: DateTime::UNIX_EPOCH,
                last_seen: DateTime::UNIX_EPOCH,
            },
            lifetime_written: ByteSize::gib(1),
            written_since_mount: ByteSize::mib(1),
        };
        vec![
            usage_of(
                "vfat",
                fat32_volume(),
                Filesystem::Other {
                    fs_type: "vfat".to_owned(),
                },
            ),
            usage_of(
                "xfs",
                VolumeIdentity::default(),
                Filesystem::Xfs {
                    inodes: PLENTY_OF_INODES,
                },
            ),
            usage_of(
                "ext4",
                VolumeIdentity::default(),
                Filesystem::Ext4 {
                    counters: ext4_counters,
                    inodes: PLENTY_OF_INODES,
                },
            ),
            usage_of(
                "btrfs",
                VolumeIdentity::default(),
                Filesystem::Btrfs(BtrfsSpace {
                    compression: Compression::Disabled,
                    unallocated: ByteSize::gib(20),
                    metadata: Allocation {
                        used: ByteSize::gib(1),
                        allocated: ByteSize::gib(2),
                    },
                    global_reserve: ByteSize::mib(512),
                    metadata_chunk: ByteSize::mib(256),
                }),
            ),
            usage_of(
                "bcachefs",
                VolumeIdentity::default(),
                Filesystem::Bcachefs(bcachefs_space()),
            ),
        ]
    }

    #[test]
    fn every_filesystem_kind_fills_its_group_with_several_rows() {
        every_filesystem_kind().iter().for_each(|usage| {
            assert!(
                !FilesystemProperty::listed_for(usage).is_empty(),
                "{} lists no property rows next to its disk usage",
                usage.mount.fs_type
            );
        });
    }

    #[test]
    fn ext4_lists_its_own_counters_and_inodes() {
        let ext4 = &every_filesystem_kind()[2];
        assert_eq!(
            FilesystemProperty::listed_for(ext4),
            vec![
                FilesystemProperty::Mount,
                FilesystemProperty::Inodes,
                FilesystemProperty::Ext4Errors,
                FilesystemProperty::Ext4Writes,
            ]
        );
    }

    #[test]
    fn bcachefs_lists_one_row_per_member_device() {
        let bcachefs = &every_filesystem_kind()[4];
        assert_eq!(
            FilesystemProperty::listed_for(bcachefs),
            vec![
                FilesystemProperty::Mount,
                FilesystemProperty::MetadataRoom,
                FilesystemProperty::Compression,
                FilesystemProperty::Member("nvme0n1p1".to_owned()),
                FilesystemProperty::Member("nvme1n1p3".to_owned()),
            ]
        );
    }

    #[test]
    fn btrfs_without_compression_hides_the_compression_row() {
        let btrfs = &every_filesystem_kind()[3];
        assert!(!FilesystemProperty::listed_for(btrfs).contains(&FilesystemProperty::Compression));
    }

    #[test]
    fn vfat_shows_its_fat_version() {
        let vfat = &every_filesystem_kind()[0];
        assert_eq!(
            FilesystemProperty::listed_for(vfat),
            vec![FilesystemProperty::Mount, FilesystemProperty::Volume]
        );
    }
}
