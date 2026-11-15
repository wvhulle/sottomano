use sottomano_desktop_sources::cpu_memory_compute::pci_gpu_readings::GpuPowerDraw;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Power, power::watt},
};

use crate::{
    clock::Clock,
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::{Section, dictionary_table},
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

fn watt_text(power: Power) -> String {
    format!("{:.0}", power.into_format_args(watt, Abbreviation))
}

pub fn gpu_power_reading_row(power: GpuPowerDraw) -> ReadingRow<'static> {
    ReadingRow {
        readout: RowReadout {
            label: ReadoutLabel::Labeled(watt_text(power.draw)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        },
        ..ReadingRow::glyph_only(
            "GPU power",
            Phosphor::Lightning(Weight::Regular).into(),
            ReadingDisplayState::Idle,
            Box::new(move |clock| gpu_power_hover_card(&power, clock)),
        )
    }
}

pub fn gpu_power_hover_card(power: &GpuPowerDraw, _clock: Clock) -> Vec<Section> {
    vec![Section::new(
        "Usage",
        dictionary_table(
            [
                Some(LabeledValue::new(
                    "Draw",
                    Quantity::power(power.draw).decimals(0),
                )),
                power
                    .limit
                    .map(|limit| LabeledValue::new("Limit", Quantity::power(limit).decimals(0))),
            ]
            .into_iter()
            .flatten(),
        ),
    )]
}
