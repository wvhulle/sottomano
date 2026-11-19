use sottomano_desktop_sources::cpu_memory_compute::temperature_thresholds::celsius_text;
use uom::si::f64::ThermodynamicTemperature;

use crate::{
    glyph_icons::phosphor::{Phosphor, Weight},
    group_indicators::{
        group_indicator::GroupIndicator,
        indicator_drawing::{IndicatorReadout, ReadoutBadge, ReadoutGauge, ReadoutLabel},
        indicator_state::IndicatorState,
    },
    hover_cards::hover_card_body::Section,
    widgets::{localized_quantities::Quantity, value_cells::LabeledValue},
};

#[derive(Clone)]
pub struct MemoryModuleTemperatureIndicator {
    pub bus_address: String,
    pub temperature: ThermodynamicTemperature,
}

impl MemoryModuleTemperatureIndicator {
    pub fn indicator<'indicator>(self) -> GroupIndicator<'indicator> {
        GroupIndicator::glyph_only(
            format!("Module {}", self.bus_address),
            Phosphor::ThermometerSimple(Weight::Regular).into(),
            IndicatorState::Idle,
        )
        .with_readout(IndicatorReadout {
            label: ReadoutLabel::Labeled(celsius_text(self.temperature)),
            gauge: ReadoutGauge::Ungauged,
            badge: ReadoutBadge::Unbadged,
        })
        .with_hover_card(Box::new(move |_| self.hover_card()))
    }

    pub fn hover_card(&self) -> Vec<Section> {
        vec![Section::hardware_identity([LabeledValue::new(
            self.bus_address.clone(),
            Quantity::temperature(self.temperature),
        )])]
    }
}
