use std::borrow::Cow;

use bytesize::ByteSize;
use chrono::Locale;
use sottomano_desktop_sources::disk_storage::{
    bcachefs_filesystem_space::BcachefsSpace,
    btrfs_filesystem_space::{BtrfsSpace, ChunkRoom},
};
use sottomano_toml_config::severity::Severity;

use crate::{
    clock::Clock,
    group_items::{
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
    fn severity(self) -> Severity {
        match self {
            MetadataPressure::Comfortable => Severity::Normal,
            MetadataPressure::Tight => Severity::Warning,
            MetadataPressure::Exhausted => Severity::Critical,
        }
    }
}

impl From<&BcachefsSpace> for MetadataPressure {
    fn from(space: &BcachefsSpace) -> Self {
        let headroom = space.metadata_headroom();
        if headroom < BCACHEFS_METADATA_CRITICAL {
            MetadataPressure::Exhausted
        } else if headroom < BCACHEFS_METADATA_WARN {
            MetadataPressure::Tight
        } else {
            MetadataPressure::Comfortable
        }
    }
}

impl From<&BtrfsSpace> for MetadataPressure {
    fn from(space: &BtrfsSpace) -> Self {
        let slack_below_reserve =
            space.metadata_slack().as_u64() < space.global_reserve.as_u64() * 2;
        if space.metadata_chunk_room() == ChunkRoom::Exhausted && slack_below_reserve {
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
}

pub(super) enum MetadataRoomRow<'a> {
    Btrfs(&'a BtrfsSpace),
    Bcachefs(&'a BcachefsSpace),
}

impl MetadataRoomRow<'_> {
    fn pressure(&self) -> MetadataPressure {
        match self {
            MetadataRoomRow::Btrfs(space) => MetadataPressure::from(*space),
            MetadataRoomRow::Bcachefs(space) => MetadataPressure::from(*space),
        }
    }

    fn room(&self) -> ByteSize {
        match self {
            MetadataRoomRow::Btrfs(space) => space.unallocated,
            MetadataRoomRow::Bcachefs(space) => space.metadata_headroom(),
        }
    }
}

impl GroupItemReading for MetadataRoomRow<'_> {
    fn name(&self) -> Cow<'static, str> {
        "Metadata room".into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::TreeStructure(Weight::Regular).into()
    }

    fn state(&self, _clock: Clock) -> ReadingDisplayState {
        self.pressure().severity().into()
    }

    fn ongoing_toast_threshold(&self) -> OngoingToastThreshold {
        OngoingToastThreshold::NotifyFromSeverity {
            minimum_severity: Severity::Critical,
            summary_text: "No room left for metadata",
        }
    }

    fn label(&self, _clock: Clock, locale: Locale) -> Option<String> {
        Some(Quantity::size(self.room()).localized_text(locale))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        let pressure = self.pressure();
        match self {
            MetadataRoomRow::Btrfs(space) => {
                let reclaim_hint = (pressure > MetadataPressure::Comfortable)
                    .then(|| LabeledValue::new("Reclaim with", BTRFS_RECLAIM_COMMAND));
                HoverCardBody::dictionary(
                    [
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
            }
            MetadataRoomRow::Bcachefs(space) => HoverCardBody::dictionary([
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
            ]),
        }
    }
}

#[cfg(test)]
mod tests {
    use sottomano_desktop_sources::disk_storage::{
        block_filesystem_mounts::Compression, btrfs_filesystem_space::Allocation,
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

    #[test]
    fn btrfs_without_room_for_a_chunk_and_little_slack_is_exhausted() {
        let space = btrfs_with(ByteSize::mib(100), ByteSize::mib(3800), ByteSize::gib(4));
        assert_eq!(MetadataPressure::from(&space), MetadataPressure::Exhausted);
    }

    #[test]
    fn btrfs_with_metadata_slack_is_only_tight() {
        let space = btrfs_with(ByteSize::mib(100), ByteSize::gib(1), ByteSize::gib(4));
        assert_eq!(MetadataPressure::from(&space), MetadataPressure::Tight);
    }

    #[test]
    fn btrfs_crowded_metadata_with_little_unallocated_is_tight() {
        let space = btrfs_with(ByteSize::gib(3), ByteSize::mib(3900), ByteSize::gib(4));
        assert_eq!(MetadataPressure::from(&space), MetadataPressure::Tight);
    }

    #[test]
    fn btrfs_with_plenty_unallocated_is_comfortable() {
        let space = btrfs_with(ByteSize::gib(50), ByteSize::mib(3900), ByteSize::gib(4));
        assert_eq!(
            MetadataPressure::from(&space),
            MetadataPressure::Comfortable
        );
    }
}
