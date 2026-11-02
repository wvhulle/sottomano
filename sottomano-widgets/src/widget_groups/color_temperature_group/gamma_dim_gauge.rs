use sottomano_sources::screen_brightness_color::gammarelay_color_temperature::ColorTemperature;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    group_items::{gauge::percent_fill, group_item_reading::GroupItemReading},
    hover_cards::hover_card_body::HoverCardBody,
    widgets::{
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        value_cells::LabeledValue,
    },
};

impl GroupItemReading for ColorTemperature {
    fn name(&self) -> &'static str {
        "Screen dimming"
    }

    fn icon(&self) -> IconGlyph {
        Phosphor::ThermometerHot(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.brightness))
    }

    fn hover_card(&self, _clock: Clock) -> HoverCardBody {
        HoverCardBody::dictionary([
            LabeledValue::new("Dim", Quantity::share(percent_fill(self.brightness))),
            LabeledValue::new("Temperature", Quantity::color_temperature(self.temperature)),
        ])
    }
}
