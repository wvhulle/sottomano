use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{f64::ThermodynamicTemperature, thermodynamic_temperature::degree_celsius},
};

use crate::{
    data::source::sysfs_sensors::temperature::Temperature,
    widgets::{
        bar_widget::{BarWidget, Glyph, Unit, Value},
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

const WARNING_CELSIUS: f64 = 70.0;
const CRITICAL_CELSIUS: f64 = 85.0;

fn celsius_text(temperature: ThermodynamicTemperature) -> String {
    format!(
        "{:.0}",
        temperature.into_format_args(degree_celsius, Abbreviation)
    )
}

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        Phosphor::ThermometerSimple(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        let average = self.average();
        if average > ThermodynamicTemperature::new::<degree_celsius>(CRITICAL_CELSIUS) {
            WidgetState::Critical
        } else if average > ThermodynamicTemperature::new::<degree_celsius>(WARNING_CELSIUS) {
            WidgetState::Warning
        } else {
            WidgetState::Idle
        }
    }

    fn value(&self) -> Option<Value> {
        let average = self.average().get::<degree_celsius>().round() as i64;
        Some(Value::Gauge {
            percent: average.min(100),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let mut chips: Vec<&str> = self.readings.iter().map(|r| r.chip.as_str()).collect();
        chips.dedup();
        let extremes = [
            self.lowest().map(|lowest| row("Min", celsius_text(lowest))),
            Some(row("Avg", celsius_text(self.average()))),
            self.highest()
                .map(|highest| row("Max", celsius_text(highest))),
        ];
        TooltipBody::rows(
            [
                row("Chips", chips.join(", ")),
                row("Samples", self.readings.len().to_string()),
            ]
            .into_iter()
            .chain(extremes.into_iter().flatten())
            .collect(),
        )
    }
}
