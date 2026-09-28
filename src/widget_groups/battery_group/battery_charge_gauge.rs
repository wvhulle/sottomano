use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{energy::watt_hour, f64::Ratio, ratio::percent},
};

use crate::{
    data::source::sysfs_sensors::battery::Battery,
    widgets::{
        bar_widget::{BarWidget, Glyph, Unit, Value},
        phosphor::Phosphor,
        tooltip_body::{TooltipBody, row},
        widget_state::WidgetState,
    },
};

impl BarWidget for Battery {
    fn icon(&self) -> Glyph {
        Phosphor::BatteryCharging(self.state().weight()).into()
    }

    fn state(&self) -> WidgetState {
        if self.capacity <= Ratio::new::<percent>(15.0) {
            WidgetState::Critical
        } else if self.capacity <= Ratio::new::<percent>(30.0) {
            WidgetState::Warning
        } else {
            WidgetState::Idle
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
