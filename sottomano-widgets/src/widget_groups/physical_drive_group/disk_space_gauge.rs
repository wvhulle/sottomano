use std::borrow::Cow;

use sottomano_desktop_sources::{
    SnapshotSource, disk_storage::whole_disk_space_occupancy::DiskSpaceOccupancy,
};
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{gauge::percent_fill, group_item_reading::GroupItemReading},
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

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
    fn name(&self) -> Cow<'static, str> {
        Self::NAME.into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::HardDrive(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.used_pct()))
    }

    fn hover_card(&self, _clock: Clock) -> Vec<Section> {
        let summary = Section::new(
            "Usage",
            dictionary_table([
                LabeledValue::new("Disk", self.disk.as_str()),
                LabeledValue::new("Used", Quantity::share(percent_fill(self.used_pct()))),
                LabeledValue::new("Free", Quantity::size(self.free())),
                LabeledValue::new("Capacity", Quantity::size(self.capacity)),
            ]),
        );
        let filesystem_shares = Section::new(
            "Used by filesystem",
            dictionary_table(self.filesystem_shares.iter().map(|share| {
                LabeledValue::new(
                    format!("{} ({})", share.mount_point.display(), share.fs_type),
                    Quantity::size(share.used_on_disk),
                )
            })),
        );
        vec![summary, filesystem_shares]
    }
}

impl GroupItemReading for MarkedDiskSpace<'_> {
    fn name(&self) -> Cow<'static, str> {
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

    fn hover_card(&self, clock: Clock) -> Vec<Section> {
        self.occupancy.hover_card(clock)
    }
}
