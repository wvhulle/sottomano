use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::Ratio, thermodynamic_temperature::kelvin},
};

use crate::{
    data::source::gamma::Gamma,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        gauge::percent_fill,
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Gamma {
    fn icon(&self) -> Glyph {
        Phosphor::ThermometerHot(self.state().weight()).into()
    }

    fn gauge(&self) -> Option<Ratio> {
        Some(percent_fill(self.brightness))
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
