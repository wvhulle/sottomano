use uom::{fmt::DisplayStyle::Abbreviation, si::thermodynamic_temperature::kelvin};

use crate::{
    data::source::gamma::Gamma,
    widgets::{
        bar_widget::{BarWidget, Glyph, Unit, Value},
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Gamma {
    fn icon(&self) -> Glyph {
        Phosphor::CircleHalf(self.state().weight()).into()
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.brightness,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row("Dim", format!("{}%", self.brightness)),
            row(
                "Temperature",
                format!(
                    "{:.0}",
                    self.temperature.into_format_args(kelvin, Abbreviation)
                ),
            ),
        ])
    }
}
