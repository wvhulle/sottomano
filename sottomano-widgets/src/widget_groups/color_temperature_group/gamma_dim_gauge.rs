use sottomano_sources::screen_brightness_color::gammarelay_color_temperature::ColorTemperature;
use uom::si::f64::Ratio;

use crate::{
    clock::Clock,
    widgets::{
        gauge::percent_fill,
        group_item_reading::GroupItemReading,
        icon_glyph::IconGlyph,
        localized_quantities::Quantity,
        phosphor::{Phosphor, Weight},
        tooltip_body::TooltipBody,
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

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            LabeledValue::new("Dim", Quantity::share(percent_fill(self.brightness))),
            LabeledValue::new("Temperature", Quantity::color_temperature(self.temperature)),
        ])
    }
}
