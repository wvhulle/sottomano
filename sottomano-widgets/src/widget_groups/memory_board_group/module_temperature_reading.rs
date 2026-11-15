use sottomano_desktop_sources::cpu_memory_compute::temperature_thresholds::celsius_text;
use uom::si::f64::ThermodynamicTemperature;

use crate::{
    group_items::{
        reading_display_state::ReadingDisplayState,
        reading_row::ReadingRow,
        source_reading::{ReadoutBadge, ReadoutGauge, ReadoutLabel, RowReadout},
    },
    hover_cards::hover_card_body::Section,
    widgets::{
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

#[derive(Clone)]
pub struct MemoryModuleTemperatureReading {
    pub bus_address: String,
    pub temperature: ThermodynamicTemperature,
}

impl MemoryModuleTemperatureReading {
    pub fn reading_row(self) -> ReadingRow<'static> {
        ReadingRow {
            readout: RowReadout {
                label: ReadoutLabel::Labeled(celsius_text(self.temperature)),
                gauge: ReadoutGauge::Ungauged,
                badge: ReadoutBadge::Unbadged,
            },
            ..ReadingRow::glyph_only(
                format!("Module {}", self.bus_address),
                Phosphor::ThermometerSimple(Weight::Regular).into(),
                ReadingDisplayState::Idle,
                Box::new(move |_| self.hover_card()),
            )
        }
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::hardware_identity([LabeledValue::new(
            self.bus_address.clone(),
            Quantity::temperature(self.temperature),
        )])]
    }
}
