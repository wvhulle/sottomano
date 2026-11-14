use std::borrow::Cow;

use chrono::Locale;
use sottomano_desktop_sources::cpu_memory_compute::temperature_thresholds::celsius_text;
use uom::si::f64::ThermodynamicTemperature;

use crate::{
    clock::Clock,
    group_items::group_item_reading::GroupItemReading,
    hover_cards::hover_card_body::{HoverCardBody, Section},
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

pub struct MemoryModuleTemperatureReading {
    pub bus_address: String,
    pub temperature: ThermodynamicTemperature,
}

impl GroupItemReading for MemoryModuleTemperatureReading {
    fn name(&self) -> Cow<'static, str> {
        format!("Module {}", self.bus_address).into()
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ThermometerSimple(Weight::Regular).into()
    }

    fn label(&self, _clock: Clock, _locale: Locale) -> Option<String> {
        Some(celsius_text(self.temperature))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::sections([Section::hardware_identity([LabeledValue::new(
            self.bus_address.clone(),
            Quantity::temperature(self.temperature),
        )])])
    }
}
