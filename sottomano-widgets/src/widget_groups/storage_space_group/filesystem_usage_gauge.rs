use bytesize::ByteSize;
use sottomano_config::severity::Severity;
use sottomano_sources::disk_storage::{
    bcachefs_filesystem_space::{BcachefsSpace, SnapshotDeletion},
    btrfs_filesystem_space::BtrfsSpace,
    mounted_filesystem_usage::{Filesystem, FilesystemUsage},
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        gauge::percent_fill,
        group_item_reading::{GroupItemReading, OngoingToastThreshold},
        hover_card_body::{HoverCardBody, Section},
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        reading_display_state::ReadingDisplayState,
        value_cells::LabeledValue,
    },
};

const FREE_WARN: ByteSize = ByteSize::gb(100);
const FREE_CRITICAL: ByteSize = ByteSize::gb(2);
const FREE_WARN_SHARE: f64 = 0.2;
const FREE_CRITICAL_SHARE: f64 = 0.05;
const BTRFS_UNALLOCATED_WARN: ByteSize = ByteSize::gib(1);
const BTRFS_CROWDED_UNALLOCATED: ByteSize = ByteSize::gib(4);
const BTRFS_CROWDED_METADATA_SHARE: f64 = 0.9;
const BTRFS_RECLAIM_COMMAND: &str = "btrfs balance start -dusage=0 /";
const BCACHEFS_METADATA_WARN: ByteSize = ByteSize::gib(8);
const BCACHEFS_METADATA_CRITICAL: ByteSize = ByteSize::gib(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataPressure {
    Comfortable,
    Tight,
    Exhausted,
}

impl MetadataPressure {
    fn of(filesystem: &Filesystem) -> Self {
        match filesystem {
            Filesystem::Btrfs(space) => Self::of_btrfs(space),
            Filesystem::Bcachefs(space) => Self::of_bcachefs(space),
            Filesystem::Other { .. } => MetadataPressure::Comfortable,
        }
    }

    fn of_btrfs(space: &BtrfsSpace) -> Self {
        let slack_below_reserve =
            space.metadata_slack().as_u64() < space.global_reserve.as_u64() * 2;
        if !space.can_allocate_metadata_chunk() && slack_below_reserve {
            return MetadataPressure::Exhausted;
        }
        let metadata_share = if space.metadata.allocated == ByteSize::default() {
            0.0
        } else {
            space.metadata.used.as_u64() as f64 / space.metadata.allocated.as_u64() as f64
        };
        let crowded = metadata_share > BTRFS_CROWDED_METADATA_SHARE
            && space.unallocated < BTRFS_CROWDED_UNALLOCATED;
        if space.unallocated < BTRFS_UNALLOCATED_WARN || crowded {
            MetadataPressure::Tight
        } else {
            MetadataPressure::Comfortable
        }
    }

    fn of_bcachefs(space: &BcachefsSpace) -> Self {
        let headroom = space.metadata_headroom();
        if headroom < BCACHEFS_METADATA_CRITICAL {
            MetadataPressure::Exhausted
        } else if headroom < BCACHEFS_METADATA_WARN {
            MetadataPressure::Tight
        } else {
            MetadataPressure::Comfortable
        }
    }

    fn severity(self) -> Severity {
        match self {
            MetadataPressure::Comfortable => Severity::Normal,
            MetadataPressure::Tight => Severity::Warning,
            MetadataPressure::Exhausted => Severity::Critical,
        }
    }
}

pub trait SpaceSeverity {
    fn free_space_severity(&self) -> Severity;
    fn metadata_pressure(&self) -> MetadataPressure;

    fn space_severity(&self) -> Severity {
        self.free_space_severity()
            .max(self.metadata_pressure().severity())
    }
}

impl SpaceSeverity for FilesystemUsage {
    fn free_space_severity(&self) -> Severity {
        let capped_by_capacity = |absolute_threshold: ByteSize, capacity_share: f64| {
            absolute_threshold.min(ByteSize::b(
                (self.total.as_u64() as f64 * capacity_share) as u64,
            ))
        };
        if self.free < capped_by_capacity(FREE_CRITICAL, FREE_CRITICAL_SHARE) {
            Severity::Critical
        } else if self.free < capped_by_capacity(FREE_WARN, FREE_WARN_SHARE) {
            Severity::Warning
        } else {
            Severity::Normal
        }
    }

    fn metadata_pressure(&self) -> MetadataPressure {
        MetadataPressure::of(&self.filesystem)
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
        self.space_severity().into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        let summary_text = if self.metadata_pressure() == MetadataPressure::Exhausted {
            "No room left for metadata"
        } else {
            "Disk almost full"
        };
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text,
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.used_pct()))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let mount_points = self
            .mount
            .mount_points
            .iter()
            .map(|mount_point| mount_point.display().to_string())
            .collect::<Vec<_>>()
            .join("  ");
        let capacity = Section::dictionary([
            LabeledValue::new("Mounted at", mount_points),
            LabeledValue::new("Used", Quantity::share(percent_fill(self.used_pct()))),
            LabeledValue::new("Free", Quantity::size(self.free)).level(self.free_space_severity()),
            LabeledValue::new("Total", Quantity::size(self.total)),
        ]);
        let pressure = self.metadata_pressure();
        let filesystem_sections = match &self.filesystem {
            Filesystem::Btrfs(space) => btrfs_sections(space, pressure),
            Filesystem::Bcachefs(space) => bcachefs_sections(space, pressure),
            Filesystem::Other { fs_type } => vec![Section::dictionary([LabeledValue::new(
                "Filesystem",
                fs_type.clone(),
            )])],
        };
        HoverCardBody::sections([capacity].into_iter().chain(filesystem_sections))
    }
}

fn btrfs_sections(space: &BtrfsSpace, pressure: MetadataPressure) -> Vec<Section> {
    let reclaim_hint = (pressure > MetadataPressure::Comfortable)
        .then(|| LabeledValue::new("Reclaim with", BTRFS_RECLAIM_COMMAND));
    vec![
        Section::dictionary(
            [
                LabeledValue::new("Compression", space.compression.to_string()),
                LabeledValue::new("Unallocated space", Quantity::size(space.unallocated))
                    .level(pressure.severity()),
                LabeledValue::new("Metadata in use", Quantity::size(space.metadata.used)),
                LabeledValue::new(
                    "Metadata reserved",
                    Quantity::size(space.metadata.allocated),
                ),
                LabeledValue::new("Global reserve", Quantity::size(space.global_reserve)),
            ]
            .into_iter()
            .chain(reclaim_hint),
        )
        .titled("Btrfs"),
    ]
}

fn bcachefs_sections(space: &BcachefsSpace, pressure: MetadataPressure) -> Vec<Section> {
    let compression = if space.background_compression == space.compression
        || space.background_compression == "none"
    {
        space.compression.clone()
    } else {
        format!(
            "{}, {} in background",
            space.compression, space.background_compression
        )
    };
    let extents = space.compressed_extents;
    let compressed_sizes = (extents.compressed > ByteSize::default())
        .then(|| {
            [
                LabeledValue::new("Before compression", Quantity::size(extents.uncompressed)),
                LabeledValue::new("After compression", Quantity::size(extents.compressed)),
            ]
        })
        .into_iter()
        .flatten();
    let snapshot_deletion = (space.snapshot_deletion == SnapshotDeletion::Running)
        .then(|| LabeledValue::new("Snapshot deletion", "in progress"));
    let summary = Section::dictionary(
        [
            LabeledValue::new(
                format!("Room for metadata at {}×", space.metadata_replicas),
                Quantity::size(space.metadata_headroom()),
            )
            .level(pressure.severity()),
            LabeledValue::new(
                "Replicas",
                format!(
                    "data {}, metadata {}",
                    space.data_replicas, space.metadata_replicas
                ),
            ),
            LabeledValue::new("Compression", compression),
        ]
        .into_iter()
        .chain(compressed_sizes)
        .chain(snapshot_deletion),
    )
    .titled("Bcachefs");
    let members = Section::dictionary(space.devices.iter().map(|device| {
        let writable_level = if device.state == "rw" {
            Severity::Normal
        } else {
            Severity::Warning
        };
        LabeledValue::new(
            format!("{} ({}, {})", device.label, device.partition, device.state),
            Quantity::size(device.free),
        )
        .level(writable_level)
    }))
    .titled("Free per member");
    vec![summary, members]
}

#[cfg(test)]
mod tests {
    use sottomano_sources::disk_storage::{
        block_filesystem_mounts::{Compression, FilesystemMount},
        btrfs_filesystem_space::Allocation,
    };

    use super::*;

    fn btrfs_with(unallocated: ByteSize, used: ByteSize, allocated: ByteSize) -> BtrfsSpace {
        BtrfsSpace {
            compression: Compression::Disabled,
            unallocated,
            metadata: Allocation { used, allocated },
            global_reserve: ByteSize::mib(512),
            metadata_chunk: ByteSize::mib(256),
        }
    }

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
            filesystem: Filesystem::Other {
                fs_type: "vfat".to_owned(),
            },
        }
    }

    #[test]
    fn small_boot_partition_mostly_free_is_normal() {
        let usage = vfat_usage_with(ByteSize::gib(2), ByteSize::mib(1700));
        assert_eq!(usage.free_space_severity(), Severity::Normal);
    }

    #[test]
    fn small_boot_partition_nearly_full_is_critical() {
        let usage = vfat_usage_with(ByteSize::gib(2), ByteSize::mib(50));
        assert_eq!(usage.free_space_severity(), Severity::Critical);
    }

    #[test]
    fn large_disk_below_absolute_thresholds_still_warns() {
        let usage = vfat_usage_with(ByteSize::tb(2), ByteSize::gb(50));
        assert_eq!(usage.free_space_severity(), Severity::Warning);
    }

    #[test]
    fn btrfs_without_room_for_a_chunk_and_little_slack_is_exhausted() {
        let space = btrfs_with(ByteSize::mib(100), ByteSize::mib(3800), ByteSize::gib(4));
        assert_eq!(
            MetadataPressure::of_btrfs(&space),
            MetadataPressure::Exhausted
        );
    }

    #[test]
    fn btrfs_with_metadata_slack_is_only_tight() {
        let space = btrfs_with(ByteSize::mib(100), ByteSize::gib(1), ByteSize::gib(4));
        assert_eq!(MetadataPressure::of_btrfs(&space), MetadataPressure::Tight);
    }

    #[test]
    fn btrfs_crowded_metadata_with_little_unallocated_is_tight() {
        let space = btrfs_with(ByteSize::gib(3), ByteSize::mib(3900), ByteSize::gib(4));
        assert_eq!(MetadataPressure::of_btrfs(&space), MetadataPressure::Tight);
    }

    #[test]
    fn btrfs_with_plenty_unallocated_is_comfortable() {
        let space = btrfs_with(ByteSize::gib(50), ByteSize::mib(3900), ByteSize::gib(4));
        assert_eq!(
            MetadataPressure::of_btrfs(&space),
            MetadataPressure::Comfortable
        );
    }
}
