use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::disk_storage::mounted_filesystem_usage::FilesystemUsage;
use sottomano_toml_config::severity::Severity;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{
        gauge::percent_fill,
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        reading_display_state::ReadingDisplayState,
    },
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
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

impl GroupItemReading for FilesystemUsage {
    fn name(&self) -> &'static str {
        "Disk usage"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Folders(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        free_space_severity(self).into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "Disk almost full",
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.used_pct()))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Used", Quantity::share(percent_fill(self.used_pct()))),
            LabeledValue::new("Free", Quantity::size(self.free)).level(free_space_severity(self)),
            LabeledValue::new("Total", Quantity::size(self.total)),
        ])
    }
}

pub(super) struct SnapshotDeletionRow;

impl GroupItemReading for SnapshotDeletionRow {
    fn name(&self) -> &'static str {
        "Snapshot deletion"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::Trash(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some("in progress".to_owned())
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Snapshot deletion", "in progress"),
            LabeledValue::new("Free space", "grows as it finishes"),
        ])
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::disk_storage::{
        block_filesystem_mounts::{Compression, FilesystemMount},
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
