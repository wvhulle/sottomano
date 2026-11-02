use sottomano_sources::disk_storage::whole_disk_space_occupancy::DiskSpaceOccupancy;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        gauge::percent_fill,
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::{Section, TooltipBody},
        value_cells::LabeledValue,
    },
};

pub(super) const DISK_SPACE_ITEM_NAME: &str = "Disk space";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskMarking {
    OnlyDisk,
    Numbered { ordinal: u8 },
}

pub struct MarkedDiskSpace<'a> {
    pub occupancy: &'a DiskSpaceOccupancy,
    pub marking: DiskMarking,
}

impl GroupItemReading for DiskSpaceOccupancy {
    fn name(&self) -> &'static str {
        DISK_SPACE_ITEM_NAME
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::HardDrive(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.used_pct()))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        let summary = Section::dictionary([
            LabeledValue::new("Disk", self.disk.as_str()),
            LabeledValue::new("Used", Quantity::share(percent_fill(self.used_pct()))),
            LabeledValue::new("Free", Quantity::size(self.free())),
            LabeledValue::new("Capacity", Quantity::size(self.capacity)),
        ]);
        let filesystem_shares = Section::dictionary(self.filesystem_shares.iter().map(|share| {
            LabeledValue::new(
                format!("{} ({})", share.mount_point.display(), share.fs_type),
                Quantity::size(share.used_on_disk),
            )
        }))
        .titled("Used by filesystem");
        TooltipBody::sections([summary, filesystem_shares])
    }
}

impl GroupItemReading for MarkedDiskSpace<'_> {
    fn name(&self) -> &'static str {
        self.occupancy.name()
    }

    fn icon(&self) -> IconGlyph {
        self.occupancy.icon()
    }

    fn identity_ordinal(&self) -> Option<u8> {
        match self.marking {
            DiskMarking::OnlyDisk => None,
            DiskMarking::Numbered { ordinal } => Some(ordinal),
        }
    }

    fn gauge(&self) -> Option<Ratio> {
        self.occupancy.gauge()
    }

    fn tooltip(&self, clock: Clock) -> TooltipBody {
        self.occupancy.tooltip(clock)
    }
}
