use sottomano_desktop_sources::disk_storage::{
    bcachefs_filesystem_space::SnapshotDeletion, mounted_filesystem_usage::Filesystem,
};

use super::{MountedFilesystem, StorageInteraction, StorageSource, StorageSpaceGroup};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::value_cells::LabeledValue,
};

struct SnapshotDeletionIndicator;

impl SnapshotDeletionIndicator {
    fn indicator<'indicator>() -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            "Snapshot deletion",
            Phosphor::Trash(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled("in progress".to_owned()),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(|_| Self::hover_card()))
    }

    fn hover_card() -> Vec<Section> {
        vec![Section::new(
            "Status",
            dictionary_table([
                LabeledValue::new("Snapshot deletion", "in progress"),
                LabeledValue::new("Free space", "grows as it finishes"),
            ]),
        )]
    }
}

impl StorageSpaceGroup {
    pub(super) fn snapshot_deletion_listing(
        filesystem: &MountedFilesystem,
    ) -> IndicatorListing<'_> {
        let deleting_snapshots = if let Some(usage) = filesystem.usage.get()
            && let Filesystem::Bcachefs(space) = &usage.filesystem
            && space.snapshot_deletion == SnapshotDeletion::Running
        {
            vec![Self::snapshot_deletion_indicator(filesystem)]
        } else {
            Vec::new()
        };
        IndicatorListing::Listed(deleting_snapshots)
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
