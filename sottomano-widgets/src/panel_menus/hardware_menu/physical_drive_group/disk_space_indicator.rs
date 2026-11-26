use sottomano_desktop_sources::{
    SnapshotSource,
    disk_storage::{
        block_filesystem_mounts::KernelDevice, whole_disk_space_occupancy::DiskSpaceOccupancy,
    },
};

use super::{
    DriveInteraction, DriveSource, HARD_DRIVES_GLYPH, PhysicalDiskSamples, PhysicalDriveGroup,
};
use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        gauge::percent_fill,
        group_indicator::GroupIndicator,
        indicator_drawing::{IdentityOrdinal, ReadoutGauge},
        indicator_group_content::IndicatorListing,
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskMarking {
    OnlyDisk,
    Numbered { ordinal: u8 },
}

pub(crate) fn disk_space_indicator(
    occupancy: &DiskSpaceOccupancy,
    marking: DiskMarking,
) -> GroupIndicator<'_> {
    let fill = percent_fill(occupancy.used_pct());
    GroupIndicator::glyph_only(
        DiskSpaceOccupancy::NAME,
        Phosphor::HardDrive(Weight::Regular).into(),
        IndicatorState::Idle,
    )
    .with_gauge(ReadoutGauge::Gauged { fill, live: fill })
    .with_ordinal(match marking {
        DiskMarking::OnlyDisk => IdentityOrdinal::Unnumbered,
        DiskMarking::Numbered { ordinal } => IdentityOrdinal::Numbered(ordinal),
    })
    .with_hover_card(Box::new(move |_| disk_space_hover_card(occupancy)))
}

pub(crate) fn disk_space_hover_card(occupancy: &DiskSpaceOccupancy) -> Vec<Section> {
    let summary = Section::new(
        "Usage",
        dictionary_table([
            LabeledValue::new("Disk", occupancy.disk.as_str()),
            LabeledValue::new("Used", Quantity::share(percent_fill(occupancy.used_pct()))),
            LabeledValue::new("Free", Quantity::size(occupancy.free())),
            LabeledValue::new("Capacity", Quantity::size(occupancy.capacity)),
        ]),
    );
    let filesystem_shares = Section::new(
        "Used by filesystem",
        dictionary_table(occupancy.filesystem_shares.iter().map(|share| {
            LabeledValue::new(
                format!("{} ({})", share.mount_point.display(), share.fs_type),
                Quantity::size(share.used_on_disk),
            )
        })),
    );
    vec![summary, filesystem_shares]
}

impl PhysicalDriveGroup {
    pub(super) fn disk_marking(&self, disk: &KernelDevice) -> DiskMarking {
        if self.disks.len() < 2 {
            return DiskMarking::OnlyDisk;
        }
        self.disks
            .keys()
            .position(|known| known == disk)
            .map(|position| u8::try_from(position + 1).expect("fewer than 255 disks are attached"))
            .map_or(DiskMarking::OnlyDisk, |ordinal| DiskMarking::Numbered {
                ordinal,
            })
    }

    pub(super) fn disk_space_listing<'view>(
        &self,
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
    ) -> IndicatorListing<'view> {
        samples
            .space
            .indicator_listing(
                |held_snapshot| vec![disk_space_indicator(held_snapshot, self.disk_marking(disk))],
                HARD_DRIVES_GLYPH.into(),
            )
            .map_indicators(|space_row| Self::wired_disk_space_indicator(disk, samples, space_row))
    }

    pub(super) fn wired_disk_space_indicator<'view>(
        disk: &'view KernelDevice,
        samples: &'view PhysicalDiskSamples,
        space_indicator: GroupIndicator<'view>,
    ) -> GroupIndicator<'view> {
        space_indicator
            .with_hover_card(Box::new(move |_| {
                samples.space.sections_described_by(disk_space_hover_card)
            }))
            .refreshed_by(DriveInteraction::Refresh(DriveSource::Space(
                disk.name.clone(),
            )))
    }
}
