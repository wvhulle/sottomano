use sottomano_sources::screen_brightness_color::gammarelay_color_temperature::ColorTemperature;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, thermodynamic_temperature::kelvin},
};

use crate::{
    clock::Clock,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        phosphor::{Phosphor, Weight},
        tooltip_body::{KeyValue, TooltipBody},
    },
};

impl BarWidget for ColorTemperature {
    fn name(&self) -> &'static str {
        "Screen dimming"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ThermometerHot(Weight::Regular).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.brightness))
    }

    fn tooltip(&self, _clock: Clock) -> TooltipBody {
        TooltipBody::dictionary([
            KeyValue::new("Dim", format!("{}%", self.brightness)),
            KeyValue::new(
                "Temperature",
                format!(
                    "{:.0}",
                    self.temperature.into_format_args(kelvin, Abbreviation)
                ),
            ),
        ])
    }
}
