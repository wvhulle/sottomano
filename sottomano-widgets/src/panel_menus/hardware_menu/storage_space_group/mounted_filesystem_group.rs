use chrono::Locale;
use sottomano_desktop_sources::disk_storage::{
    bcachefs_filesystem_space::SnapshotDeletion, mounted_filesystem_usage::Filesystem,
};

use super::{
    FILESYSTEM_USAGE_GLYPH, FilesystemProperty, MountedFilesystem, SUBVOLUME_USAGE_GLYPH,
    StorageInteraction, StorageSource, StorageSpaceGroup,
    filesystem_usage_gauge::{
        SnapshotDeletionIndicator, filesystem_usage_hover_card, filesystem_usage_indicator,
    },
    mount_group_title,
    subvolume_usage_indicator::{SubvolumeUsageIndicator, subvolume_usage_hover_card},
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::{IndicatorGroupContent, IndicatorListing},
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
};

impl StorageSpaceGroup {
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

    fn subvolume_indicators(
        filesystem: &MountedFilesystem,
        locale: Locale,
    ) -> impl Iterator<Item = GroupIndicator<'_>> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        filesystem
            .usage
            .get()
            .zip(filesystem.subvolume_usage.get())
            .into_iter()
            .flat_map(move |(usage, subvolume_usage)| {
                subvolume_usage.subvolumes.iter().map(move |subvolume| {
                    SubvolumeUsageIndicator {
                        subvolume,
                        pool_total: usage.total,
                    }
                    .indicator(locale)
                    .with_hover_card(Box::new(move |_| {
                        filesystem.subvolume_usage.hover_card_with(
                            SubvolumeUsageIndicator {
                                subvolume,
                                pool_total: usage.total,
                            }
                            .hover_card(),
                        )
                    }))
                    .refreshed_by(StorageInteraction::Refresh(StorageSource::SubvolumeUsage(
                        primary_mount_point.to_path_buf(),
                    )))
                })
            })
    }

    pub(super) fn mount_group(
        filesystem: &MountedFilesystem,
        locale: Locale,
    ) -> IndicatorGroupContent<'_> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        let headline_indicator = {
            Self::wired_filesystem_usage_indicator(
                filesystem,
                filesystem.usage.indicator(
                    |held_snapshot| filesystem_usage_indicator(held_snapshot),
                    FILESYSTEM_USAGE_GLYPH.into(),
                ),
            )
            .with_headline_glyph(Phosphor::Archive(Weight::Regular).into())
        };
        let usage_listing = filesystem
            .usage
            .indicator_listing(
                |held_snapshot| vec![filesystem_usage_indicator(held_snapshot)],
                FILESYSTEM_USAGE_GLYPH.into(),
            )
            .map_indicators(|usage_row| {
                Self::wired_filesystem_usage_indicator(filesystem, usage_row)
            });
        let subvolume_indicators: Vec<GroupIndicator<'_>> =
            Self::subvolume_indicators(filesystem, locale).collect();
        let subvolume_listing = match filesystem.subvolume_usage.failure() {
            Some(failure) => IndicatorListing::Incomplete {
                listed_indicators: subvolume_indicators,
                unread_sources: vec![Self::subvolume_failure_indicator(
                    filesystem,
                    failure.indicator(SUBVOLUME_USAGE_GLYPH.into()),
                )],
            },
            None => IndicatorListing::Listed(subvolume_indicators),
        };
        let snapshot_deletion_indicators = if let Some(usage) = filesystem.usage.get()
            && let Filesystem::Bcachefs(space) = &usage.filesystem
            && space.snapshot_deletion == SnapshotDeletion::Running
        {
            vec![Self::snapshot_deletion_indicator(filesystem)]
        } else {
            Vec::new()
        };
        let owned_mount_point = primary_mount_point.to_path_buf();
        IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(
                IndicatorGroupFamily::StorageSpace,
                primary_mount_point.display().to_string(),
            ),
            mount_group_title(primary_mount_point),
            headline_indicator,
            &filesystem.expansion,
            move |expansion| StorageInteraction::MountExpansion {
                mount_point: owned_mount_point.clone(),
                expansion,
            },
            [
                usage_listing,
                IndicatorListing::Listed(Self::property_indicators(filesystem, locale).collect()),
                subvolume_listing,
                IndicatorListing::Listed(snapshot_deletion_indicators),
            ]
            .into_iter()
            .collect(),
        )
    }

    fn wired_filesystem_usage_indicator<'view>(
        filesystem: &'view MountedFilesystem,
        usage_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        usage_indicator
            .with_hover_card(Box::new(move |_| {
                filesystem
                    .usage
                    .sections_described_by(filesystem_usage_hover_card)
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::Usage(
                filesystem.mount.primary_mount_point().to_path_buf(),
            )))
    }

    fn subvolume_failure_indicator<'view>(
        filesystem: &'view MountedFilesystem,
        failure_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        failure_indicator
            .with_hover_card(Box::new(move |_| {
                filesystem
                    .subvolume_usage
                    .sections_described_by(subvolume_usage_hover_card)
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::SubvolumeUsage(
                filesystem.mount.primary_mount_point().to_path_buf(),
            )))
    }

    fn snapshot_deletion_indicator(filesystem: &MountedFilesystem) -> GroupIndicator<'_> {
        SnapshotDeletionIndicator::indicator()
            .with_hover_card(Box::new(move |_| {
                filesystem
                    .usage
                    .hover_card_with(SnapshotDeletionIndicator::hover_card())
            }))
            .refreshed_by(StorageInteraction::Refresh(StorageSource::Usage(
                filesystem.mount.primary_mount_point().to_path_buf(),
            )))
    }
}
