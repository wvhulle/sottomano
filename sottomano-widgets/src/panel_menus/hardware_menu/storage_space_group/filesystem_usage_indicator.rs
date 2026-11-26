use bytesize::ByteSize;
use sottomano_desktop_sources::{
    SnapshotSource, disk_storage::mounted_filesystem_usage::FilesystemUsage,
};
use sottomano_toml_config::severity::Severity;

use super::{
    FILESYSTEM_USAGE_GLYPH, MountedFilesystem, StorageInteraction, StorageSource, StorageSpaceGroup,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill, group_indicator::GroupIndicator, indicator_drawing::ReadoutGauge,
        indicator_group_content::IndicatorListing,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        ongoing_toast::{NotificationBody, NotificationProgress, StateToast},
        value_cells::LabeledValue,
    },
};

const FREE_WARN: ByteSize = ByteSize::gb(100);
const FREE_CRITICAL: ByteSize = ByteSize::gb(2);
const FREE_WARN_SHARE: f64 = 0.2;
const FREE_CRITICAL_SHARE: f64 = 0.05;

fn free_space_severity(usage: &FilesystemUsage) -> Severity {
    let capped_by_capacity = |absolute_threshold: ByteSize, capacity_share: f64| {
        absolute_threshold.min(ByteSize::b(
            (usage.total.as_u64() as f64 * capacity_share) as u64,
        ))
    };
    if usage.free < capped_by_capacity(FREE_CRITICAL, FREE_CRITICAL_SHARE) {
        Severity::Critical
    } else if usage.free < capped_by_capacity(FREE_WARN, FREE_WARN_SHARE) {
        Severity::Warning
    } else {
        Severity::Normal
    }
}

pub(crate) fn filesystem_usage_indicator(usage: &FilesystemUsage) -> GroupIndicator<'_> {
    let fill = percent_fill(usage.used_pct());
    GroupIndicator::glyph_only(
        FilesystemUsage::NAME,
        Phosphor::Folders(Weight::Regular).into(),
        free_space_severity(usage).into(),
    )
    .with_gauge(ReadoutGauge::Gauged { fill, live: fill })
    .with_state_toast(StateToast::NotifyFromSeverity {
        minimum_severity: Severity::Critical,
        summary_text: "Disk almost full",
        body: NotificationBody::Bodiless,
        progress: NotificationProgress::Measured(fill),
    })
    .with_hover_card(Box::new(move |_| filesystem_usage_hover_card(usage)))
}

pub(crate) fn filesystem_usage_hover_card(usage: &FilesystemUsage) -> Vec<Section> {
    vec![Section::new(
        "Space",
        dictionary_table([
            LabeledValue::new("Used", Quantity::share(percent_fill(usage.used_pct()))),
            LabeledValue::new("Free", Quantity::size(usage.free)).level(free_space_severity(usage)),
            LabeledValue::new("Total", Quantity::size(usage.total)),
        ]),
    )]
}

impl StorageSpaceGroup {
    pub(super) fn filesystem_usage_listing(filesystem: &MountedFilesystem) -> IndicatorListing<'_> {
        filesystem
            .usage
            .indicator_listing(
                |held_snapshot| vec![filesystem_usage_indicator(held_snapshot)],
                FILESYSTEM_USAGE_GLYPH.into(),
            )
            .map_indicators(|usage_row| {
                Self::wired_filesystem_usage_indicator(filesystem, usage_row)
            })
    }

    pub(super) fn wired_filesystem_usage_indicator<'view>(
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
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::disk_storage::{
        block_filesystem_mounts::{FilesystemMount, mount_compression_options::Compression},
        mounted_filesystem_usage::{Filesystem, MountAccess},
        udev_volume_identity::VolumeIdentity,
    };

    use super::*;

    fn vfat_usage_with(total: ByteSize, free: ByteSize) -> FilesystemUsage {
        FilesystemUsage {
            mount: FilesystemMount {
                devices: Vec::new(),
                mount_points: vec!["/boot".into()],
                fs_type: "vfat".to_owned(),
                compression: Compression::Disabled,
            },
            total,
            free,
            block_size: ByteSize::kib(4),
            access: MountAccess::ReadWrite,
            volume: VolumeIdentity::default(),
            filesystem: Filesystem::Other {
                fs_type: "vfat".to_owned(),
            },
        }
    }

    #[test]
    fn small_boot_partition_mostly_free_is_normal() {
        let usage = vfat_usage_with(ByteSize::gib(2), ByteSize::mib(1700));
        assert_eq!(free_space_severity(&usage), Severity::Normal);
    }

    #[test]
    fn small_boot_partition_nearly_full_is_critical() {
        let usage = vfat_usage_with(ByteSize::gib(2), ByteSize::mib(50));
        assert_eq!(free_space_severity(&usage), Severity::Critical);
    }

    #[test]
    fn large_disk_below_absolute_thresholds_still_warns() {
        let usage = vfat_usage_with(ByteSize::tb(2), ByteSize::gb(50));
        assert_eq!(free_space_severity(&usage), Severity::Warning);
    }
}
