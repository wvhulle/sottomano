use chrono::Locale;

use super::{
    FILESYSTEM_USAGE_GLYPH, MountedFilesystem, StorageInteraction, StorageSpaceGroup,
    filesystem_usage_indicator::filesystem_usage_indicator, mount_group_title,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
};

impl StorageSpaceGroup {
    pub(super) fn mount_group(
        filesystem: &MountedFilesystem,
        locale: Locale,
    ) -> IndicatorGroupContent<'_> {
        let primary_mount_point = filesystem.mount.primary_mount_point();
        let headline_indicator = Self::wired_filesystem_usage_indicator(
            filesystem,
            filesystem.usage.indicator(
                |held_snapshot| filesystem_usage_indicator(held_snapshot),
                FILESYSTEM_USAGE_GLYPH.into(),
            ),
        )
        .with_headline_glyph(Phosphor::Archive(Weight::Regular).into());
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
                Self::filesystem_usage_listing(filesystem),
                Self::filesystem_property_listing(filesystem, locale),
                Self::subvolume_usage_listing(filesystem, locale),
                Self::snapshot_deletion_listing(filesystem),
            ]
            .into_iter()
            .collect(),
        )
    }
}
