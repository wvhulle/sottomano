use sottomano_sources::gamma::Gamma;
use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, thermodynamic_temperature::kelvin},
};

use crate::widgets::{
    bar_widget::{BarWidget, Glyph},
    gauge::percent_fill,
    phosphor::Phosphor,
    tooltip_body::{KeyValue, TooltipBody},
};

impl BarWidget for Gamma {
    fn name(&self) -> &'static str {
        "Screen dimming"
    }

    fn icon(&self) -> Glyph {
        Phosphor::ThermometerHot(self.state().weight()).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.brightness))
    }

    fn tooltip(&self) -> TooltipBody {
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
