use sottomano_desktop_sources::{
    SnapshotSource,
    disk_storage::disk_io_activity::{DiskActivity, DiskTransfer, DiskTransferDirection},
};
use uom::si::{
    f64::{InformationRate, Ratio},
    information_rate::{gibibyte_per_second, kibibyte_per_second},
    ratio::ratio,
};

use crate::{
    group_indicators::{
        indicator_drawing::ReadoutGauge,
        indicator_state::IndicatorState,
        reading_row::{PanelVisibility, ReadingRow},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

const BACKGROUND_TRANSFER_KIB_PER_S: f64 = 64.0;
const SCALE_END_GIB_PER_S: f64 = 4.0;

fn background_rate() -> InformationRate {
    InformationRate::new::<kibibyte_per_second>(BACKGROUND_TRANSFER_KIB_PER_S)
}

fn transfer_icon(transfer: &DiskTransfer) -> IconGlyph {
    let direction_icon = match transfer.direction {
        DiskTransferDirection::Read => Phosphor::TrayArrowUp(Weight::Regular),
        DiskTransferDirection::Write => Phosphor::TrayArrowDown(Weight::Regular),
    };
    direction_icon.with_weight(Weight::Regular).into()
}

fn transfer_fill(transfer: &DiskTransfer) -> Ratio {
    let scale_start = background_rate();
    let scale_end = InformationRate::new::<gibibyte_per_second>(SCALE_END_GIB_PER_S);
    if transfer.rate <= scale_start {
        return Ratio::new::<ratio>(0.0);
    }
    let scale_width = (scale_end / scale_start).get::<ratio>().ln();
    let position = (transfer.rate / scale_start).get::<ratio>().ln();
    Ratio::new::<ratio>((position / scale_width).clamp(0.0, 1.0))
}

pub(crate) fn disk_transfer_indicator(transfer: &DiskTransfer) -> ReadingRow<'_> {
    let name = match transfer.direction {
        DiskTransferDirection::Read => "Disk reads",
        DiskTransferDirection::Write => "Disk writes",
    };
    let state = if transfer.rate <= background_rate() {
        IndicatorState::Off
    } else {
        IndicatorState::Idle
    };
    let fill = transfer_fill(transfer);
    let mut row = ReadingRow::glyph_only(
        name,
        transfer_icon(transfer),
        state,
        Box::new(move |_| disk_transfer_hover_card(transfer)),
    );
    row.readout.gauge = ReadoutGauge::Gauged { fill, live: fill };
    row
}

pub(crate) fn disk_activity_indicator(activity: &DiskActivity) -> ReadingRow<'_> {
    ReadingRow {
        panel_visibility: PanelVisibility::Hidden,
        ..ReadingRow::glyph_only(
            DiskActivity::NAME,
            transfer_icon(&activity.read),
            IndicatorState::Idle,
            Box::new(move |_| disk_transfer_hover_card(&activity.read)),
        )
    }
}

pub(crate) fn disk_transfer_hover_card(transfer: &DiskTransfer) -> Vec<Section> {
    let rate_label = match transfer.direction {
        DiskTransferDirection::Read => "Read",
        DiskTransferDirection::Write => "Write",
    };
    vec![Section::new(
        "Throughput",
        dictionary_table([
            LabeledValue::new("Disk", transfer.disk.clone()),
            LabeledValue::new(rate_label, Quantity::transfer_rate(transfer.rate)),
            LabeledValue::new("Disk busy", Quantity::share(transfer.disk_busy)),
        ]),
    )]
}
