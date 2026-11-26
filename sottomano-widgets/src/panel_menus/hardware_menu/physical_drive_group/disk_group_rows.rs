use chrono::Locale;
use sottomano_desktop_sources::disk_storage::block_filesystem_mounts::KernelDevice;

use super::{
    DriveInteraction, HARD_DRIVES_GLYPH, PhysicalDiskSamples, PhysicalDriveGroup, disk_group_title,
    disk_space_indicator::disk_space_indicator,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_group_content::IndicatorGroupContent,
        indicator_group_identity::{IndicatorGroupFamily, IndicatorGroupIdentity},
    },
};

impl PhysicalDriveGroup {
    pub(super) fn disk_group<'view>(
        &'view self,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        locale: Locale,
    ) -> IndicatorGroupContent<'view> {
        let headline_indicator = self.disk_space_headline(disk, samples);
        let indicator_listing = [
            self.disk_space_listing(disk, samples),
            Self::bcachefs_member_listing(disk, samples, locale),
            Self::drive_health_listing(disk, samples),
            Self::disk_activity_listing(disk, samples),
            Self::disk_transfer_listing(disk, samples),
        ]
        .into_iter()
        .collect();
        IndicatorGroupContent::expandable_instance(
            IndicatorGroupIdentity::durable(IndicatorGroupFamily::PhysicalDrive, disk.name.clone()),
            disk_group_title(disk),
            headline_indicator,
            &samples.expansion,
            move |expansion| DriveInteraction::DiskExpansion {
                disk: disk.clone(),
                expansion,
            },
            indicator_listing,
        )
    }

    fn disk_space_headline<'view>(
        &self,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) -> GroupIndicator<'view> {
        Self::wired_disk_space_indicator(
            disk,
            samples,
            samples.space.indicator(
                |held_snapshot| disk_space_indicator(held_snapshot, self.disk_marking(disk)),
                HARD_DRIVES_GLYPH.into(),
            ),
        )
        .with_headline_glyph(Phosphor::Cylinder(Weight::Regular).into())
    }
}
