use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{energy::watt_hour, f64::Ratio, ratio::percent},
};

use crate::{
    data::source::sysfs_sensors::battery::Battery,
    widgets::{
        bar_widget::{BarWidget, Glyph, Level, Unit, Value},
        phosphor,
        tooltip_body::{TooltipBody, row},
    },
};

impl BarWidget for Battery {
    fn icon(&self) -> Glyph {
        phosphor::BatteryCharging::Regular.into()
    }

    fn level(&self) -> Level {
        if self.capacity <= Ratio::new::<percent>(15.0) {
            Level::Critical
        } else if self.capacity <= Ratio::new::<percent>(30.0) {
            Level::Warning
        } else {
            Level::Ok
        }
    }

    fn value(&self) -> Option<Value> {
        Some(Value::Gauge {
            percent: self.capacity.get::<percent>().round() as i64,
            unit: Unit::Percent,
        })
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::rows(vec![
            row(
                "Battery",
                format!(
                    "{:.0}",
                    self.capacity.into_format_args(percent, Abbreviation)
                ),
            ),
            row("Status", self.status.as_str()),
            row(
                "Remaining",
                format!(
                    "{:.1}",
                    self.energy.into_format_args(watt_hour, Abbreviation)
                ),
            ),
        ])
    }
}
