use crate::{
    data::source::sysfs_sensors::{favg, fmax, fmin, temperature::Temperature},
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        font_awesome::Solid,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Temperature {
    fn icon(&self) -> Glyph {
        Solid::TemperatureHalf.into()
    }

    fn level(&self) -> Level {
        let average = favg(&self.celsius_samples());
        if average > 85.0 {
            Level::Critical
        } else if average > 70.0 {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        let average = favg(&self.celsius_samples()).round() as i64;
        Some(Value::Gauge {
            percent: average.min(100),
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        let samples = self.celsius_samples();
        let mut chips: Vec<&str> = self.readings.iter().map(|r| r.chip.as_str()).collect();
        chips.dedup();
        TooltipBody::rows(vec![
            row("Chips", chips.join(", ")),
            row("Samples", self.readings.len().to_string()),
            row("Min", format!("{}°C", fmin(&samples).round() as i64)),
            row("Avg", format!("{}°C", favg(&samples).round() as i64)),
            row("Max", format!("{}°C", fmax(&samples).round() as i64)),
        ])
    }
}
