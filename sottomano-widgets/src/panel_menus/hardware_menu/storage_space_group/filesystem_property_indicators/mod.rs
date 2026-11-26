mod compression_setting_indicator;
mod ext4_error_count_indicator;
mod ext4_lifetime_writes_indicator;
mod inode_usage_indicator;
mod metadata_room_indicator;
mod mount_access_indicator;
mod volume_identity_indicator;

use chrono::Locale;
use sottomano_desktop_sources::disk_storage::mounted_filesystem_usage::{
    Filesystem, FilesystemUsage,
};

use self::{
    compression_setting_indicator::CompressionSettingIndicator,
    ext4_error_count_indicator::Ext4ErrorCountIndicator,
    ext4_lifetime_writes_indicator::Ext4LifetimeWritesIndicator,
    inode_usage_indicator::InodeUsageIndicator, metadata_room_indicator::MetadataRoomIndicator,
    mount_access_indicator::MountAccessIndicator,
    volume_identity_indicator::VolumeIdentityIndicator,
};
use super::{MountedFilesystem, StorageInteraction, StorageSource, StorageSpaceGroup};
use crate::{
    group_indicators::{
        group_indicator::GroupIndicator, indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::Section,
};

const LISTED_PROPERTIES: [FilesystemProperty; 7] = [
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
    Ext4Errors,
    Ext4Writes,
}

impl FilesystemProperty {
    fn listed_for(usage: &FilesystemUsage) -> Vec<FilesystemProperty> {
        LISTED_PROPERTIES
            .into_iter()
            .filter(|property| property.row(usage).is_some())
            .collect()
    }

    fn row<'view>(
        &self,
        usage: &'view FilesystemUsage,
    ) -> Option<FilesystemPropertyIndicator<'view>> {
        match (self, &usage.filesystem) {
            (FilesystemProperty::Mount, _) => {
                Some(FilesystemPropertyIndicator::Mount(MountAccessIndicator {
                    usage,
                }))
            }
            (FilesystemProperty::Volume, _) if !usage.volume.properties.is_empty() => Some(
                FilesystemPropertyIndicator::Volume(VolumeIdentityIndicator { usage }),
            ),
            (
                FilesystemProperty::Inodes,
                Filesystem::Ext4 { inodes, .. } | Filesystem::Xfs { inodes },
            ) if inodes.total > 0 => {
                Some(FilesystemPropertyIndicator::Inodes(InodeUsageIndicator {
                    inodes: *inodes,
                }))
            }
            (FilesystemProperty::MetadataRoom, Filesystem::Btrfs(space)) => Some(
                FilesystemPropertyIndicator::MetadataRoom(MetadataRoomIndicator::Btrfs(space)),
            ),
            (FilesystemProperty::MetadataRoom, Filesystem::Bcachefs(space)) => Some(
                FilesystemPropertyIndicator::MetadataRoom(MetadataRoomIndicator::Bcachefs(space)),
            ),
            (FilesystemProperty::Compression, Filesystem::Btrfs(space)) => {
                enabled_compression(CompressionSettingIndicator::Btrfs(&space.compression))
            }
            (FilesystemProperty::Compression, Filesystem::Bcachefs(space)) => {
                enabled_compression(CompressionSettingIndicator::Bcachefs(space))
            }
            (FilesystemProperty::Ext4Errors, Filesystem::Ext4 { counters, .. }) => Some(
                FilesystemPropertyIndicator::Ext4Errors(Ext4ErrorCountIndicator {
                    recorded_errors: counters.recorded_errors,
                }),
            ),
            (FilesystemProperty::Ext4Writes, Filesystem::Ext4 { counters, .. }) => Some(
                FilesystemPropertyIndicator::Ext4Writes(Ext4LifetimeWritesIndicator {
                    counters: *counters,
                }),
            ),
            _ => None,
        }
    }
}

fn enabled_compression(
    compression_indicator: CompressionSettingIndicator<'_>,
) -> Option<FilesystemPropertyIndicator<'_>> {
    compression_indicator
        .is_enabled()
        .then_some(FilesystemPropertyIndicator::Compression(
            compression_indicator,
        ))
}

enum FilesystemPropertyIndicator<'view> {
    Mount(MountAccessIndicator<'view>),
    Volume(VolumeIdentityIndicator<'view>),
    Inodes(InodeUsageIndicator),
    MetadataRoom(MetadataRoomIndicator<'view>),
    Compression(CompressionSettingIndicator<'view>),
    Ext4Errors(Ext4ErrorCountIndicator),
    Ext4Writes(Ext4LifetimeWritesIndicator),
}

impl<'view> FilesystemPropertyIndicator<'view> {
    fn group_indicator(self, locale: Locale) -> GroupIndicator<'view> {
        match self {
            FilesystemPropertyIndicator::Mount(row) => row.indicator(),
            FilesystemPropertyIndicator::Volume(row) => row.indicator(),
            FilesystemPropertyIndicator::Inodes(row) => row.indicator(locale),
            FilesystemPropertyIndicator::MetadataRoom(row) => row.indicator(locale),
            FilesystemPropertyIndicator::Compression(row) => row.indicator(locale),
            FilesystemPropertyIndicator::Ext4Errors(row) => row.indicator(locale),
            FilesystemPropertyIndicator::Ext4Writes(row) => row.indicator(locale),
        }
    }

    fn hover_card(&self) -> Vec<Section> {
        match self {
            FilesystemPropertyIndicator::Mount(row) => row.hover_card(),
            FilesystemPropertyIndicator::Volume(row) => row.hover_card(),
            FilesystemPropertyIndicator::Inodes(row) => row.hover_card(),
            FilesystemPropertyIndicator::MetadataRoom(row) => row.hover_card(),
            FilesystemPropertyIndicator::Compression(row) => row.hover_card(),
            FilesystemPropertyIndicator::Ext4Errors(row) => row.hover_card(),
            FilesystemPropertyIndicator::Ext4Writes(row) => row.hover_card(),
        }
    }
}

impl StorageSpaceGroup {
    pub(super) fn filesystem_property_listing(
        filesystem: &MountedFilesystem,
        locale: Locale,
    ) -> IndicatorListing<'_> {
        IndicatorListing::Listed(Self::property_indicators(filesystem, locale).collect())
    }

    fn property_indicators(
        filesystem: &MountedFilesystem,
        locale: Locale,
    ) -> impl Iterator<Item = GroupIndicator<'_>> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        filesystem.usage.get().into_iter().flat_map(move |usage| {
            FilesystemProperty::listed_for(usage)
                .into_iter()
                .filter_map(move |property| {
                    let row = property.row(usage)?;
                    let card_property = property.clone();
                    Some(
                        row.group_indicator(locale)
                            .with_hover_card(Box::new(move |_| {
                                card_property
                                    .row(usage)
                                    .map_or_else(Vec::new, |card_indicator| {
                                        filesystem
                                            .usage
                                            .hover_card_with(card_indicator.hover_card())
                                    })
                            }))
                            .refreshed_by(StorageInteraction::Refresh(StorageSource::Usage(
                                primary_mount_point.to_path_buf(),
                            ))),
                    )
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use bytesize::ByteSize;
    use chrono::DateTime;
    use sottomano_desktop_sources::disk_storage::{
        bcachefs_filesystem_space::{
            AllowedDeviceData, BcachefsDeviceSpace, BcachefsSpace, CompressedExtents,
            SnapshotDeletion,
        },
        block_filesystem_mounts::{FilesystemMount, mount_compression_options::Compression},
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

    fn mounted_filesystem_usage(
        fs_type: &str,
        volume: VolumeIdentity,
        filesystem: Filesystem,
    ) -> FilesystemUsage {
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
            allowed_data: AllowedDeviceData::DataAndMetadata,
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
            mounted_filesystem_usage(
                "vfat",
                fat32_volume(),
                Filesystem::Other {
                    fs_type: "vfat".to_owned(),
                },
            ),
            mounted_filesystem_usage(
                "xfs",
                VolumeIdentity::default(),
                Filesystem::Xfs {
                    inodes: PLENTY_OF_INODES,
                },
            ),
            mounted_filesystem_usage(
                "ext4",
                VolumeIdentity::default(),
                Filesystem::Ext4 {
                    counters: ext4_counters,
                    inodes: PLENTY_OF_INODES,
                },
            ),
            mounted_filesystem_usage(
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
            mounted_filesystem_usage(
                "bcachefs",
                VolumeIdentity::default(),
                Filesystem::Bcachefs(bcachefs_space()),
            ),
        ]
    }

    #[test]
    fn every_filesystem_kind_fills_its_group_with_several_indicators() {
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
    fn bcachefs_lists_metadata_and_compression() {
        let bcachefs = &every_filesystem_kind()[4];
        assert_eq!(
            FilesystemProperty::listed_for(bcachefs),
            vec![
                FilesystemProperty::Mount,
                FilesystemProperty::MetadataRoom,
                FilesystemProperty::Compression,
            ]
        );
    }

    #[test]
    fn btrfs_without_compression_hides_the_compression_indicator() {
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
