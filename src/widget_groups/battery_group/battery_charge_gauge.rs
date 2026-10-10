use uom::{
    fmt::DisplayStyle::Abbreviation,
    si::{energy::watt_hour, f64::Ratio, ratio::percent},
};

use crate::{
    data::source::sysfs_sensors::battery::Battery,
    widgets::{
        bar_widget::{BarWidget, Glyph},
        phosphor::Phosphor,
        tooltip_body::{KeyValue, Section, TooltipBody},
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

    fn gauge(&self) -> Option<Ratio> {
        Some(self.capacity)
    }

    fn tooltip(&self) -> TooltipBody {
        TooltipBody::sections(&[Section::untitled_dictionary(
            [
                Some(KeyValue::new(
                    "Battery",
                    format!(
                        "{:.0}",
                        self.capacity.into_format_args(percent, Abbreviation)
                    ),
                )),
                self.status
                    .map(|status| KeyValue::new("Status", status.as_str())),
                Some(KeyValue::new(
                    "Remaining",
                    format!(
                        "{:.1}",
                        self.energy.into_format_args(watt_hour, Abbreviation)
                    ),
                )),
            ]
            .into_iter()
            .flatten(),
        )])
    }
}
