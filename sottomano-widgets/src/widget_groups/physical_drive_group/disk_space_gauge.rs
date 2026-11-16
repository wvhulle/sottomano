use sottomano_desktop_sources::{
    SnapshotSource, disk_storage::whole_disk_space_occupancy::DiskSpaceOccupancy,
};

use crate::{
    group_indicators::{
        gauge::percent_fill,
        indicator_drawing::{IdentityOrdinal, ReadoutGauge},
        indicator_state::IndicatorState,
        reading_row::ReadingRow,
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
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

pub(crate) fn disk_space_indicator(
    occupancy: &DiskSpaceOccupancy,
    marking: DiskMarking,
) -> ReadingRow<'_> {
    let fill = percent_fill(occupancy.used_pct());
    let mut row = ReadingRow::glyph_only(
        DiskSpaceOccupancy::NAME,
        Phosphor::HardDrive(Weight::Regular).into(),
        IndicatorState::Idle,
        Box::new(move |_| disk_space_hover_card(occupancy)),
    );
    row.readout.gauge = ReadoutGauge::Gauged { fill, live: fill };
    row.face.ordinal = match marking {
        DiskMarking::OnlyDisk => IdentityOrdinal::Unnumbered,
        DiskMarking::Numbered { ordinal } => IdentityOrdinal::Numbered(ordinal),
    };
    row
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
